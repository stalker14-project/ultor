pub mod commands;
pub mod sponsor;

use crate::bot::commands::{DiscordCommandHandler, DiscordCommandResponse};
use crate::services::{BotDatabaseService, ServicesContainer};
use crate::{config_get, config_get_array, error::Error};
use log::{debug, error, info};
use serenity::all::{
    Command, CreateCommand, CreateInteractionResponse, CreateInteractionResponseMessage, GuildId,
    Interaction, Ready,
};
use serenity::async_trait;
use serenity::prelude::*;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct DiscordApp {
    registrations: Vec<CreateCommand>,
    global_registrations: Vec<CreateCommand>,
    guilds: Vec<GuildId>,

    handlers_map: BTreeMap<String, Arc<dyn DiscordCommandHandler + Send + Sync>>,
    handlers: Vec<Arc<dyn DiscordCommandHandler + Send + Sync>>,

    db: Arc<BotDatabaseService>,
    sponsor_task_started: AtomicBool,
}

#[async_trait]
impl EventHandler for DiscordApp {
    async fn ready(&self, ctx: Context, ready: Ready) {
        let bot_name = ready.user.name.as_str();

        info!("{} connected to discord API. Working...", bot_name);
        info!(
            "Registering {} commands...",
            self.registrations.len() + self.global_registrations.len()
        );
        debug!("Guilds: {:?}", ready.guilds);

        for guild in &self.guilds {
            let commands = guild
                .set_commands(&ctx.http, self.registrations.clone())
                .await;
            if let Err(why) = commands {
                error!("Error pushing commands to {}: {}", guild, why);
                continue;
            }

            debug!(
                "Registered {} commands to {}",
                commands.unwrap().len(),
                guild
            );
        }

        let commands =
            Command::set_global_commands(&ctx.http, self.global_registrations.clone()).await;
        if let Err(why) = &commands {
            error!("Error pushing global commands: {}", why);
        }

        debug!("Registered {} global commands", commands.unwrap().len());
        info!("Finished commands registering. Listening for incoming interactions...");

        // `ready` can fire more than once (e.g. on reconnect); only spawn the
        // sponsor watcher on the first invocation.
        if self
            .sponsor_task_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            tokio::spawn(sponsor::run_sponsor_loop(
                ctx.clone(),
                Arc::clone(&self.db),
                self.guilds.clone(),
            ));
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(cmd) = interaction {
            debug!(
                "Received `command` interaction from {}, command: {}. Processing...",
                cmd.user.name, cmd.data.name
            );

            // check for allowed guilds
            if let Some(guild_id) = cmd.guild_id {
                if !self.guilds.contains(&guild_id) {
                    if let Err(e) = cmd
                        .create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("Not allowed at this guild."),
                            ),
                        )
                        .await
                    {
                        error!("Error sending followup command: {e}");
                    }
                }
            }

            if cmd.guild_id.is_none() {
                if let Err(e) = cmd
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new().content("Not allowed in DMs."),
                        ),
                    )
                    .await
                {
                    error!("Error sending followup command: {e}");
                }
            }

            let handler = self.handlers_map.get(&cmd.data.name);
            if handler.is_none() {
                let result = cmd
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new().content("Command not found!"),
                        ),
                    )
                    .await;

                if let Err(e) = result {
                    error!("Error responding to discord: {e}");
                }

                return;
            }

            let handler = handler.unwrap();

            // 2 branches
            // deferred and default
            if handler.definition().is_deferred {
                if let Err(e) = cmd
                    .create_response(
                        &ctx.http,
                        CreateInteractionResponse::Defer(
                            CreateInteractionResponseMessage::new()
                                .content("Your request is processing...")
                                .ephemeral(handler.definition().is_ephemeral),
                        ),
                    )
                    .await
                {
                    error!("Error creating defer response: {e}");
                    return;
                }

                let opts = &cmd.data.options();
                let response = handler.handler(opts).await;
                match response {
                    DiscordCommandResponse::Default(_) => {
                        error!("Deferred command returned default response!");
                    }
                    DiscordCommandResponse::Followup(response) => {
                        if let Err(e) = cmd.create_followup(&ctx.http, response).await {
                            error!("Error sending followup command: {e}");
                        }
                    }
                }
                return;
            }

            let opts = &cmd.data.options();
            let response = handler.handler(opts).await;
            match response {
                DiscordCommandResponse::Default(response) => {
                    if let Err(e) = cmd.create_response(&ctx.http, response).await {
                        error!("Error sending followup command: {e}");
                    }
                }
                DiscordCommandResponse::Followup(_) => {
                    error!("Default command returned deferred response!");
                }
            }
        }
    }
}

impl DiscordApp {
    pub fn new(
        command_defs: Vec<Arc<dyn DiscordCommandHandler + Send + Sync>>,
        services: &ServicesContainer,
    ) -> Result<Self, crate::error::Error> {
        let guilds: Vec<&str> = config_get_array!("discord.guilds", as_array, as_str).unwrap();

        let mut app = Self {
            registrations: vec![],
            global_registrations: vec![],
            guilds: Vec::with_capacity(guilds.len()),
            handlers: vec![],
            handlers_map: BTreeMap::new(),
            db: services.get_unsafe(),
            sponsor_task_started: AtomicBool::new(false),
        };

        app.construct_commands(command_defs)?;
        app.populate_registrations()?;
        app.populate_guilds()?;

        Ok(app)
    }

    pub async fn start(self) -> Result<(), Error> {
        let token = config_get!("discord.token", as_str).unwrap();

        let mut client = Client::builder(token, GatewayIntents::empty())
            .event_handler(self)
            .await?;

        client.start().await?;
        Ok(())
    }

    fn construct_commands(
        &mut self,
        commands: Vec<Arc<dyn DiscordCommandHandler + Send + Sync>>,
    ) -> Result<(), Error> {
        for command in commands {
            self.handlers_map
                .insert(command.definition().name.to_string(), Arc::clone(&command));
            self.handlers.push(command);
        }

        debug!("Constructed commands: {:?}", self.handlers);
        debug!("Constructed commands tree: {:?}", self.handlers_map);

        Ok(())
    }

    fn populate_registrations(&mut self) -> Result<(), Error> {
        for command in &self.handlers {
            if command.definition().is_global {
                self.global_registrations.push(command.registration());
            } else {
                self.registrations.push(command.registration());
            }
        }

        Ok(())
    }

    fn populate_guilds(&mut self) -> Result<(), Error> {
        let guilds: Vec<&str> = config_get_array!("discord.guilds", as_array, as_str).unwrap();

        for guild_id in guilds {
            let guild_id = guild_id.parse::<u64>()?;
            let guild_id = GuildId::from(guild_id);
            self.guilds.push(guild_id);
        }

        Ok(())
    }
}
