#![allow(unused)]

use log::error;
use serenity::{
    all::{
        CommandOptionType, CreateCommand, CreateCommandOption, ResolvedOption, ResolvedValue, User,
        UserId,
    },
    async_trait,
};

use crate::{
    extract_discord_arg,
    services::{SS14AuthClientService, SS14DatabaseService, ServicesContainer},
    try_discord_unwrap,
    utils::{format_extra_data, gen_random_color, RED_COLOR},
};

use super::{DiscordCommandDefinition, DiscordCommandHandler, DiscordCommandResponse};

#[derive(Debug)]
pub struct LinkCommand {
    ss14_client: std::sync::Arc<SS14AuthClientService>,
    ss14_db: std::sync::Arc<SS14DatabaseService>,
}

impl LinkCommand {
    pub fn new(services: &ServicesContainer) -> Self {
        Self {
            ss14_client: services.get_unsafe(),
            ss14_db: services.get_unsafe(),
        }
    }
}

#[async_trait]
impl DiscordCommandHandler for LinkCommand {
    fn definition(&self) -> DiscordCommandDefinition {
        DiscordCommandDefinition::new_global("link", true, true)
    }

    fn registration(&self) -> CreateCommand {
        CreateCommand::new("link")
            .name_localized("ru", "привязка")
            .description("Operates with discord and SS14 links")
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::SubCommandGroup,
                    "discord",
                    "Groups all discord interactions with the link",
                )
                .name_localized("ru", "дискорд")
                .description_localized("ru", "Группирует все дискорд взаимодействия с привязкой")
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::SubCommand,
                        "status",
                        "Displays the status of the link",
                    )
                    .name_localized("ru", "статус")
                    .description_localized("ru", "Показывает статус привязки")
                    .add_sub_option(
                        CreateCommandOption::new(
                            CommandOptionType::User,
                            "user",
                            "User to get link of",
                        )
                        .required(true),
                    ),
                )
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::SubCommand,
                        "unlink",
                        "Unlinks specific user",
                    )
                    .name_localized("ru", "отвязать")
                    .description_localized("ru", "Отвязывает пользователя")
                    .add_sub_option(
                        CreateCommandOption::new(CommandOptionType::User, "user", "User to unlink")
                            .name_localized("ru", "пользователь")
                            .description_localized("ru", "Пользователь, которого отвязать"),
                    ),
                ),
            )
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::SubCommandGroup,
                    "ss14",
                    "Groups all ss14 interactions to the link",
                )
                .name_localized("ru", "игра")
                .description_localized("ru", "Группирует все ин-гейм взаимодействия с привязкой")
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::SubCommand,
                        "status",
                        "Displays the status of the link",
                    )
                    .name_localized("ru", "статус")
                    .description_localized("ru", "Показывает статус привязки")
                    .add_sub_option(
                        CreateCommandOption::new(
                            CommandOptionType::String,
                            "login",
                            "In-Game Login",
                        )
                        .name_localized("ru", "логин")
                        .description_localized("ru", "Внутриигровой логин")
                        .required(true),
                    ),
                )
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::SubCommand,
                        "unlink",
                        "Unlinks specific user",
                    )
                    .name_localized("ru", "отвязать")
                    .description_localized("ru", "Отвязывает пользователя")
                    .add_sub_option(
                        CreateCommandOption::new(
                            CommandOptionType::String,
                            "login",
                            "Login to unlink",
                        )
                        .name_localized("ru", "логин")
                        .description_localized("ru", "Логин пользователя, которого отвязать"),
                    ),
                ),
            )
    }

    async fn handler(&self, opts: &[ResolvedOption]) -> DiscordCommandResponse {
        let command = map_command(opts);

        let command =
            try_discord_unwrap!(command, none => "No command supplied", ephemeral => true);

        match command {
            LinkSubCommand::Discord { command } => match command {
                LinkDiscordSubCommand::Status(u) => {
                    let user_id = u.id;
                    let ss14_user_id = try_discord_unwrap!(
                        self.ss14_client.get_user_id_from_discord(user_id.to_string()).await,
                        none => "🔍 No linked SS14 account found for this user.",
                        error => "❌ An error occurred while fetching UUID.",
                        log => "Failed to get UID by Discord ID.",
                        ephemeral => true
                    );

                    let in_game_login = try_discord_unwrap!(
                        self.ss14_db.get_login(ss14_user_id).await,
                        none => "❔ User has no known in-game login.",
                        error => "❌ An error occurred while fetching login.",
                        log => "Failed to get login by Discord ID.",
                        ephemeral => true
                    );

                    let extra_data =
                        format_extra_data(&user_id.to_string(), &self.ss14_client).await;
                    let extra_data = try_discord_unwrap!(
                        extra_data,
                        error => "❌ An error occurred while fetching extra data.",
                        log => "Failed to get extra data by Discord ID.",
                        ephemeral => true
                    );

                    DiscordCommandResponse::followup_embed_response(
                        &format!(
                            "🧑‍🚀 **In-Game Login:** `{}`\n🧾 **Extra Data:** \n{}",
                            in_game_login, extra_data
                        ),
                        None,
                        Some(gen_random_color()),
                        true,
                    )
                }
                LinkDiscordSubCommand::Unlink(u) => {
                    let result = self
                        .ss14_client
                        .delete_record("discord".to_string(), u.id.to_string())
                        .await;
                    match result {
                        Ok(Some(_)) => DiscordCommandResponse::followup_response(
                            "Successfully unlinked account.",
                            true,
                        ),
                        Ok(None) => DiscordCommandResponse::followup_response(
                            "No such linked account exist.",
                            true,
                        ),
                        Err(e) => {
                            let err_id = uuid::Uuid::new_v4();
                            error!("{}. Failed to delete record. Error: {}", err_id, e);
                            DiscordCommandResponse::followup_response(
                                &format!(
                                    "Error occurred while trying to delete record.\nError ID: {}",
                                    err_id
                                ),
                                true,
                            )
                        }
                    }
                }
            },
            LinkSubCommand::SS14 { command } => match command {
                LinkSS14SubCommand::Status(u) => {
                    let ss14_user_id = try_discord_unwrap!(
                        self.ss14_client.get_user_id(u.clone()).await,
                        none => "User not found",
                        error => "Error occured during fetching UserID",
                        log => "Error. ",
                        ephemeral => true
                    );

                    let discord_uid = try_discord_unwrap!(
                        self.ss14_client.get_discord_id(ss14_user_id).await,
                        none => "User is not linked",
                        error => "Error occured during fetching discord ID",
                        log => "Error. ",
                        ephemeral => true
                    );

                    let discord_uid = UserId::new(discord_uid.parse().unwrap());

                    let extra_data =
                        format_extra_data(&discord_uid.to_string(), &self.ss14_client).await;
                    let extra_data = try_discord_unwrap!(
                        extra_data,
                        error => "❌ An error occurred while fetching extra data.",
                        log => "Failed to get extra data by Discord ID.",
                        ephemeral => true
                    );

                    DiscordCommandResponse::followup_embed_response(
                        &format!(
                            "🧑‍🚀 **In-Game Login:** `{}`\n🧾 **Extra Data:** \n{}",
                            u, extra_data
                        ),
                        None,
                        Some(gen_random_color()),
                        true,
                    )
                }
                LinkSS14SubCommand::Unlink(u) => {
                    let uuid = try_discord_unwrap!(
                        self.ss14_client.get_user_id(u).await,
                        none => "User not found",
                        error => "Error occured during UserID fetch",
                        log => "Error: ",
                        ephemeral => true
                    );
                    let result = self
                        .ss14_client
                        .delete_record("uid".to_string(), uuid.to_string())
                        .await;
                    match result {
                        Ok(Some(_)) => DiscordCommandResponse::followup_response(
                            "Successfully unlinked account.",
                            true,
                        ),
                        Ok(None) => DiscordCommandResponse::followup_response(
                            "No such linked account exist.",
                            true,
                        ),
                        Err(e) => {
                            let err_id = uuid::Uuid::new_v4();
                            error!("{}. Failed to delete record. Error: {}", err_id, e);
                            DiscordCommandResponse::followup_response(
                                &format!(
                                    "Error occurred while trying to delete record.\nError ID: {}",
                                    err_id
                                ),
                                true,
                            )
                        }
                    }
                }
            },
        }
    }
}

enum LinkSubCommand {
    Discord { command: LinkDiscordSubCommand },
    SS14 { command: LinkSS14SubCommand },
}

enum LinkDiscordSubCommand {
    Status(User),
    Unlink(User),
}

enum LinkSS14SubCommand {
    Status(String),
    Unlink(String),
}

fn map_command(opts: &[ResolvedOption]) -> Option<LinkSubCommand> {
    for group in opts {
        let (group_name, group_opts) = match (group.name, &group.value) {
            ("discord", ResolvedValue::SubCommandGroup(opts)) => ("discord", opts),
            ("ss14", ResolvedValue::SubCommandGroup(opts)) => ("ss14", opts),
            _ => continue,
        };

        let sub = group_opts.first()?;
        let (sub_name, sub_opts) = match (sub.name, &sub.value) {
            (name, ResolvedValue::SubCommand(opts)) => (name, opts),
            _ => continue,
        };

        let arg = sub_opts.first();

        let command = match (group_name, sub_name) {
            // DISCORD
            ("discord", "status") => {
                let user = match &arg?.value {
                    ResolvedValue::User(u, _) => u,
                    _ => continue,
                };
                LinkSubCommand::Discord {
                    command: LinkDiscordSubCommand::Status((*user).clone()),
                }
            }

            ("discord", "unlink") => {
                let user = match &arg?.value {
                    ResolvedValue::User(u, _) => u,
                    _ => continue,
                };

                LinkSubCommand::Discord {
                    command: LinkDiscordSubCommand::Unlink((*user).clone()),
                }
            }

            // SS14
            ("ss14", "status") => {
                let val = match &arg?.value {
                    ResolvedValue::String(s) => s,
                    _ => continue,
                };
                LinkSubCommand::SS14 {
                    command: LinkSS14SubCommand::Status(val.to_string()),
                }
            }

            ("ss14", "unlink") => {
                let val = match &arg?.value {
                    ResolvedValue::String(s) => s,
                    _ => continue,
                };
                LinkSubCommand::SS14 {
                    command: LinkSS14SubCommand::Unlink(val.to_string()),
                }
            }

            _ => continue,
        };

        return Some(command);
    }

    None
}
