pub mod bot;
pub mod config;
pub mod error;
pub mod services;
pub mod utils;

pub use bot::DiscordApp;
pub use config::ConfigBuilder;
pub use config::CONFIG;
pub use error::Error;

pub async fn initialize_services(container: &services::ServicesContainer) -> Result<(), Error> {
    use services::{BotDatabaseService, SS14AuthClientService, SS14DatabaseService};
    let bot_db_path = config_get!("database.bot_database_path", as_str).unwrap();

    let db_service =
        BotDatabaseService::new(bot_db_path.to_string(), "./migrations".to_string()).await?;
    container.register(db_service);

    let ss14_db_uri = config_get!("database.ss14_database_url", as_str).unwrap();
    container.register(SS14DatabaseService::new(ss14_db_uri.to_string())?);

    let discord_auth_uri = config_get!("auth.discord_auth_uri", as_str).unwrap();
    let discord_auth_token = config_get!("auth.discord_auth_token", as_str).unwrap();
    let ss14_auth_uri = config_get!("auth.ss14_auth_uri", as_str).unwrap();
    container.register(SS14AuthClientService::new(
        discord_auth_uri.to_string(),
        discord_auth_token.to_string(),
        ss14_auth_uri.to_string(),
    )?);

    Ok(())
}

pub fn command_definitions(
    services: &services::ServicesContainer,
) -> Vec<std::sync::Arc<dyn bot::commands::DiscordCommandHandler + Send + Sync>> {
    use bot::commands::*;
    use std::sync::Arc;

    vec![
        Arc::new(PingCommand),
        Arc::new(FemboyCommand),
        Arc::new(UserIdCommand::new(services)),
        Arc::new(SummonCommand::new(services)),
        Arc::new(LinkCommand::new(services)),
        Arc::new(PreferencesCommand::new(services)),
    ]
}
