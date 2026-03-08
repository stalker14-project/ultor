use std::sync::Arc;

use crate::services::{SS14AuthClientService, SS14DatabaseService, ServicesContainer};
use crate::try_discord_unwrap;
use crate::utils::gen_random_color;

use super::*;
use serenity::{
    all::{CommandOptionType, CreateCommandOption},
    async_trait,
};

#[derive(Debug)]
pub struct PreferencesCommand {
    ss14_client: Arc<SS14AuthClientService>,
    ss14_db: Arc<SS14DatabaseService>,
}

impl PreferencesCommand {
    pub fn new(services: &ServicesContainer) -> Self {
        Self {
            ss14_db: services.get_unsafe(),
            ss14_client: services.get_unsafe(),
        }
    }
}

#[async_trait]
impl DiscordCommandHandler for PreferencesCommand {
    fn definition(&self) -> DiscordCommandDefinition {
        DiscordCommandDefinition::new_global("preferences", true, true)
    }

    fn registration(&self) -> CreateCommand {
        CreateCommand::new("preferences")
            .name_localized("ru", "профиль")
            .description("Controls the player's profile")
            .description_localized("ru", "Управляет профилем игрока")
            .default_member_permissions(MANAGE_WEBHOOKS_SERVER_PERMISSION)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::SubCommand,
                    "delete",
                    "Removes the player's profile",
                )
                .name_localized("ru", "удалить")
                .description_localized("ru", "Удаляет профиль игрока")
                .add_sub_option(
                    CreateCommandOption::new(
                        CommandOptionType::String,
                        "login",
                        "In-game player's login",
                    )
                    .description_localized("ru", "Внутриигровой логин игрока")
                    .required(true),
                ),
            )
    }

    async fn handler(&self, opts: &[ResolvedOption]) -> DiscordCommandResponse {
        let delete_cmd = opts.iter().find(|x| x.name == "delete");
        if delete_cmd.is_none() {
            return DiscordCommandResponse::followup_response("Invalid command", true);
        }
        let delete_cmd = delete_cmd.unwrap();
        log::trace!("{:?}", delete_cmd);
        if let ResolvedValue::SubCommand(opts) = &delete_cmd.value {
            let login = try_discord_unwrap!(
                opts_get_login(opts),
                none => "Login is not specified",
                ephemeral => true
            );
            let user_id = try_discord_unwrap!(
                self.ss14_client.get_user_id(login.clone()).await,
                none => "Login is not specified",
                error => "Error occurred during UID fetch.",
                log => "Failed to get user ID.",
                ephemeral => true
            );

            match self.ss14_db.delete_pref(user_id).await {
                Ok(_) => return DiscordCommandResponse::followup_embed_response("Success!", None, Some(gen_random_color()), true),
                Err(e) => {
                    let log_id = uuid::Uuid::new_v4();
                    log::error!(
                        "Failed to delete user's preferences for: {login}/{user_id}. Error: {e}.\nLog ID: {log_id}"
                    );
                    return DiscordCommandResponse::followup_embed_response(
                        "Failed to delete user's preferences. Check server logs for more info.",
                        Some(&log_id.to_string()),
                        Some(Color::RED),
                        true,
                    );
                }
            }
        }

        return DiscordCommandResponse::followup_response("Invalid command passed", true);
    }
}
