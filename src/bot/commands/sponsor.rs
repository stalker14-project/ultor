use std::sync::Arc;

use serenity::{
    all::{CommandOptionType, CreateCommand, CreateCommandOption, ResolvedOption, ResolvedValue},
    async_trait,
};

use crate::extract_discord_arg;
use crate::services::{BotDatabaseService, ServicesContainer};
use crate::try_discord_unwrap;
use crate::utils::{gen_random_color, now_unix, parse_duration_secs, sponsor_roles};

use super::{DiscordCommandDefinition, DiscordCommandHandler, DiscordCommandResponse};

#[derive(Debug)]
pub struct SponsorCommand {
    db: Arc<BotDatabaseService>,
}

impl SponsorCommand {
    pub fn new(services: &ServicesContainer) -> Self {
        Self {
            db: services.get_unsafe(),
        }
    }
}

#[async_trait]
impl DiscordCommandHandler for SponsorCommand {
    fn definition(&self) -> DiscordCommandDefinition {
        DiscordCommandDefinition::new_local("give-sponsor", true, true)
    }

    fn registration(&self) -> CreateCommand {
        // Build the role picker out of the configured sponsor roles. Only one
        // role can be granted per invocation.
        let mut role_option = CreateCommandOption::new(
            CommandOptionType::String,
            "role",
            "Which sponsor role to grant",
        )
        .name_localized("ru", "роль")
        .description_localized("ru", "Какую роль спонсора выдать")
        .required(true);

        for (name, id) in sponsor_roles() {
            role_option = role_option.add_string_choice(name, id.to_string());
        }

        CreateCommand::new("give-sponsor")
            .name_localized("ru", "выдать-спонсорку")
            .description("Grants a sponsor role to a user")
            .description_localized("ru", "Выдаёт пользователю роль спонсора")
            .default_member_permissions(super::MANAGE_WEBHOOKS_SERVER_PERMISSION)
            .add_option(
                CreateCommandOption::new(CommandOptionType::User, "user", "User to grant sponsor to")
                    .name_localized("ru", "пользователь")
                    .description_localized("ru", "Пользователь, которому выдать спонсорку")
                    .required(true),
            )
            .add_option(role_option)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "duration",
                    "How long it lasts (e.g. 30d, 12h, 1w, 1h30m). Empty = forever",
                )
                .name_localized("ru", "срок")
                .description_localized(
                    "ru",
                    "Срок действия (например 30d, 12h, 1w, 1h30m). Пусто = навсегда",
                )
                .required(false),
            )
    }

    async fn handler(&self, opts: &[ResolvedOption]) -> DiscordCommandResponse {
        let user = try_discord_unwrap!(
            opts.iter().find_map(|opt| match (opt.name, &opt.value) {
                ("user", ResolvedValue::User(user, _)) => Some(*user),
                _ => None,
            }),
            none => "User is not specified",
            ephemeral => true
        );

        let role_id = try_discord_unwrap!(
            extract_discord_arg!(opts, "role", String),
            none => "Role is not specified",
            ephemeral => true
        );
        // The value comes from a predefined choice, but guard against a
        // non-numeric one just in case.
        try_discord_unwrap!(
            role_id.parse::<u64>().ok(),
            none => "The selected role is invalid.",
            ephemeral => true
        );

        let expires_at = match extract_discord_arg!(opts, "duration", String) {
            Some(raw) if !raw.trim().is_empty() => {
                let secs = try_discord_unwrap!(
                    parse_duration_secs(raw.trim()),
                    none => "Invalid duration format. Use e.g. `30d`, `12h`, `1w`, or leave it empty for a permanent sponsorship.",
                    ephemeral => true
                );
                Some(now_unix() + secs)
            }
            _ => None,
        };

        let discord_id = user.id.to_string();
        try_discord_unwrap!(
            self.db
                .upsert_sponsorship(&discord_id, &role_id, expires_at, now_unix())
                .await,
            error => "Failed to store the sponsorship. Check server logs for more info.",
            log => "Failed to upsert sponsorship",
            ephemeral => true
        );

        let message = match expires_at {
            Some(ts) => format!(
                "✅ Granted <@&{}> to <@{}>.\n⏳ Active until <t:{}:F> (<t:{}:R>).\nThe role is issued automatically within a few minutes.",
                role_id, discord_id, ts, ts
            ),
            None => format!(
                "✅ Granted **permanent** <@&{}> to <@{}>.\nThe role is issued automatically within a few minutes.",
                role_id, discord_id
            ),
        };

        DiscordCommandResponse::followup_embed_response(
            &message,
            None,
            Some(gen_random_color()),
            true,
        )
    }
}
