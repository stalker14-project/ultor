pub mod femboy;
pub mod link;
pub mod ping;
pub mod summon;
pub mod user_id;
pub mod preferences;

pub use femboy::FemboyCommand;
pub use link::LinkCommand;
pub use ping::PingCommand;
pub use summon::SummonCommand;
pub use user_id::UserIdCommand;
pub use preferences::PreferencesCommand;

use serenity::all::{
    Color, CreateCommand, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseFollowup, CreateInteractionResponseMessage, Permissions,
    ResolvedOption, ResolvedValue,
};
use serenity::async_trait;

const MANAGE_WEBHOOKS_SERVER_PERMISSION: Permissions =
    Permissions::from_bits(0x20 | 0x20000000).unwrap();

#[derive(Debug)]
pub enum DiscordCommandResponse {
    Default(CreateInteractionResponse),
    Followup(CreateInteractionResponseFollowup),
}

impl DiscordCommandResponse {
    pub fn default_response(s: &str, ephemeral: bool) -> Self {
        Self::Default(CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content(s.to_owned())
                .ephemeral(ephemeral),
        ))
    }

    pub fn default_embed_response(
        content: &str,
        footer: Option<&str>,
        color: Option<Color>,
        ephemeral: bool,
    ) -> Self {
        let mut embed = CreateEmbed::new().description(content.to_owned());

        if let Some(content) = footer {
            embed = embed.footer(CreateEmbedFooter::new(content));
        }

        if let Some(content) = color {
            embed = embed.color(content);
        }

        Self::Default(CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .embed(embed)
                .ephemeral(ephemeral),
        ))
    }

    pub fn followup_response(s: &str, ephemeral: bool) -> Self {
        Self::Followup(
            CreateInteractionResponseFollowup::new()
                .content(s.to_owned())
                .ephemeral(ephemeral),
        )
    }

    pub fn followup_embed_response(
        content: &str,
        footer: Option<&str>,
        color: Option<Color>,
        ephemeral: bool,
    ) -> Self {
        let mut embed = CreateEmbed::new().description(content.to_owned());

        if let Some(content) = footer {
            embed = embed.footer(CreateEmbedFooter::new(content));
        }

        if let Some(content) = color {
            embed = embed.color(content);
        }

        Self::Followup(
            CreateInteractionResponseFollowup::new()
                .embed(embed)
                .ephemeral(ephemeral),
        )
    }
}

#[async_trait]
pub trait DiscordCommandHandler: Send + Sync + std::fmt::Debug {
    fn definition(&self) -> DiscordCommandDefinition;
    fn registration(&self) -> CreateCommand;
    async fn handler(&self, opts: &[ResolvedOption]) -> DiscordCommandResponse;
}

/// Represents some settings for discord commands.
///
/// (name, is_global, is_deferred)
pub struct DiscordCommandDefinition {
    pub name: &'static str,
    pub is_global: bool,
    pub is_deferred: bool,
    pub is_ephemeral: bool,
}

impl DiscordCommandDefinition {
    pub fn new(name: &'static str, is_global: bool, is_deferred: bool, is_ephemeral: bool) -> Self {
        Self {
            name,
            is_deferred,
            is_global,
            is_ephemeral,
        }
    }

    pub fn new_global(name: &'static str, is_deferred: bool, is_ephemeral: bool) -> Self {
        Self {
            name,
            is_global: true,
            is_deferred,
            is_ephemeral,
        }
    }

    pub fn new_local(name: &'static str, is_deferred: bool, is_ephemeral: bool) -> Self {
        Self {
            name,
            is_global: false,
            is_deferred,
            is_ephemeral,
        }
    }
}

fn opts_get_login(opts: &[ResolvedOption]) -> Option<String> {
    let mut login: Option<String> = None;

    for option in opts {
        if let ("login", ResolvedValue::String(val)) = (option.name, &option.value) {
            login = Some(val.to_string());
            break;
        }
    }

    login
}
