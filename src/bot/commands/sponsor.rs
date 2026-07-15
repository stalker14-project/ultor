use std::sync::Arc;

use serenity::{
    all::{
        CommandOptionType, CreateCommand, CreateCommandOption, GuildId, ResolvedOption,
        ResolvedValue, RoleId,
    },
    async_trait,
    http::Http,
};

use crate::services::{BotDatabaseService, ServicesContainer};
use crate::utils::{gen_random_color, now_unix, parse_duration_secs, sponsor_roles, RED_COLOR};
use crate::{config_get, config_get_array, extract_discord_arg, try_discord_unwrap};

use super::{DiscordCommandDefinition, DiscordCommandHandler, DiscordCommandResponse};

/// Shared setup for the sponsor commands: the database handle, a standalone
/// REST client and the configured guild list.
fn sponsor_command_parts(
    services: &ServicesContainer,
) -> (Arc<BotDatabaseService>, Arc<Http>, Vec<GuildId>) {
    let token = config_get!("discord.token", as_str).unwrap();

    let guilds = config_get_array!("discord.guilds", as_array, as_str)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|s| s.parse::<u64>().ok())
        .map(GuildId::new)
        .collect();

    (services.get_unsafe(), Arc::new(Http::new(token)), guilds)
}

/// Builds the role picker out of the configured sponsor roles.
fn build_role_option(
    description: &str,
    description_ru: &str,
    required: bool,
) -> CreateCommandOption {
    let mut option = CreateCommandOption::new(CommandOptionType::String, "role", description)
        .name_localized("ru", "роль")
        .description_localized("ru", description_ru)
        .required(required);

    for (name, id) in sponsor_roles() {
        option = option.add_string_choice(name, id.to_string());
    }

    option
}

#[derive(Debug)]
pub struct SponsorCommand {
    db: Arc<BotDatabaseService>,
    /// Standalone REST client so the role can be granted right away, without
    /// waiting for the background watcher's next tick.
    http: Arc<Http>,
    guilds: Vec<GuildId>,
}

impl SponsorCommand {
    pub fn new(services: &ServicesContainer) -> Self {
        let (db, http, guilds) = sponsor_command_parts(services);
        Self { db, http, guilds }
    }
}

#[async_trait]
impl DiscordCommandHandler for SponsorCommand {
    fn definition(&self) -> DiscordCommandDefinition {
        DiscordCommandDefinition::new_local("give-sponsor", true, true)
    }

    fn registration(&self) -> CreateCommand {
        // Only one role can be granted per invocation.
        let role_option = build_role_option(
            "Which sponsor role to grant",
            "Какую роль спонсора выдать",
            true,
        );

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
        let role = RoleId::new(try_discord_unwrap!(
            role_id.parse::<u64>().ok(),
            none => "The selected role is invalid.",
            ephemeral => true
        ));

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

        // Grant the role right away instead of waiting for the watcher. If the
        // user isn't a member of a configured guild the call fails there; the
        // watcher will pick it up once they join.
        let mut granted = false;
        for guild in &self.guilds {
            match self
                .http
                .add_member_role(*guild, user.id, role, Some("Sponsorship granted"))
                .await
            {
                Ok(_) => granted = true,
                Err(e) => log::debug!("Couldn't grant sponsor role in {guild} immediately: {e}"),
            }
        }

        let sync_note = if granted {
            "The role has been assigned."
        } else {
            "The role will be assigned automatically once the user is on the server."
        };

        let message = match expires_at {
            Some(ts) => format!(
                "✅ Granted <@&{}> to <@{}>.\n⏳ Active until <t:{}:F> (<t:{}:R>).\n{}",
                role_id, discord_id, ts, ts, sync_note
            ),
            None => format!(
                "✅ Granted **permanent** <@&{}> to <@{}>.\n{}",
                role_id, discord_id, sync_note
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

#[derive(Debug)]
pub struct RemoveSponsorCommand {
    db: Arc<BotDatabaseService>,
    /// Standalone REST client so the role can be revoked right away, without
    /// waiting for the background watcher's next tick.
    http: Arc<Http>,
    guilds: Vec<GuildId>,
}

impl RemoveSponsorCommand {
    pub fn new(services: &ServicesContainer) -> Self {
        let (db, http, guilds) = sponsor_command_parts(services);
        Self { db, http, guilds }
    }
}

#[async_trait]
impl DiscordCommandHandler for RemoveSponsorCommand {
    fn definition(&self) -> DiscordCommandDefinition {
        DiscordCommandDefinition::new_local("remove-sponsor", true, true)
    }

    fn registration(&self) -> CreateCommand {
        // Unlike granting, the role is optional here: an empty pick means
        // "revoke every sponsorship this user has".
        let role_option = build_role_option(
            "Which sponsor role to revoke. Empty = all of them",
            "Какую роль спонсора снять. Пусто = все",
            false,
        );

        CreateCommand::new("remove-sponsor")
            .name_localized("ru", "снять-спонсорку")
            .description("Revokes a sponsor role from a user")
            .description_localized("ru", "Снимает с пользователя роль спонсора")
            .default_member_permissions(super::MANAGE_WEBHOOKS_SERVER_PERMISSION)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::User,
                    "user",
                    "User to revoke sponsor from",
                )
                .name_localized("ru", "пользователь")
                .description_localized("ru", "Пользователь, с которого снять спонсорку")
                .required(true),
            )
            .add_option(role_option)
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

        let requested_role = extract_discord_arg!(opts, "role", String);
        let discord_id = user.id.to_string();

        let stored = try_discord_unwrap!(
            self.db.get_user_sponsorships(&discord_id).await,
            error => "Failed to read the sponsorships. Check server logs for more info.",
            log => "Failed to fetch user sponsorships",
            ephemeral => true
        );

        let targets: Vec<_> = match &requested_role {
            Some(role_id) => stored
                .into_iter()
                .filter(|s| &s.role_id == role_id)
                .collect(),
            None => stored,
        };

        if targets.is_empty() {
            let message = match &requested_role {
                Some(role_id) => format!("<@{}> doesn't have <@&{}>.", discord_id, role_id),
                None => format!("<@{}> has no sponsorships.", discord_id),
            };

            return DiscordCommandResponse::followup_embed_response(
                &message,
                None,
                Some(RED_COLOR),
                true,
            );
        }

        let mut removed_anywhere = false;
        for sponsorship in &targets {
            // A malformed role id can't be revoked on Discord's side, but the
            // record still has to go.
            match sponsorship.role_id.parse::<u64>() {
                Ok(id) => {
                    let role = RoleId::new(id);
                    for guild in &self.guilds {
                        match self
                            .http
                            .remove_member_role(*guild, user.id, role, Some("Sponsorship revoked"))
                            .await
                        {
                            Ok(_) => removed_anywhere = true,
                            Err(e) => log::debug!(
                                "Couldn't revoke sponsor role {role} in {guild} immediately: {e}"
                            ),
                        }
                    }
                }
                Err(_) => log::error!(
                    "Invalid role id stored in sponsorships table: {}",
                    sponsorship.role_id
                ),
            }

            try_discord_unwrap!(
                self.db
                    .remove_sponsorship(&discord_id, &sponsorship.role_id)
                    .await,
                error => "Failed to remove the sponsorship. Check server logs for more info.",
                log => "Failed to remove sponsorship",
                ephemeral => true
            );
        }

        let roles = targets
            .iter()
            .map(|s| format!("<@&{}>", s.role_id))
            .collect::<Vec<_>>()
            .join(", ");

        let sync_note = if removed_anywhere {
            "The role has been taken away."
        } else {
            "Couldn't take the role away on the server (the user may not be a member) — the record has been deleted regardless."
        };

        let message = format!(
            "✅ Revoked {} from <@{}>.\n{}",
            roles, discord_id, sync_note
        );

        DiscordCommandResponse::followup_embed_response(
            &message,
            None,
            Some(gen_random_color()),
            true,
        )
    }
}
