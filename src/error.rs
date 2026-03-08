use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Deserialize error: {0}")]
    DeserializationError(#[from] serde_json::Error),
    #[error("Discord bot error: {0}")]
    BotError(String),
    #[error("Discord API error: {0}")]
    SerenityError(Box<serenity::Error>),
    #[error("Type mismatch error: {0}")]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    #[error("Migration error: {0}")]
    MigrationError(#[from] sqlx::migrate::MigrateError),
    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Invalid uuid error: {0}")]
    InvalidUuidError(#[from] uuid::Error),
    #[error("TypeAuthD Error: {0}")]
    TypeAuthdError(String),
}

impl From<serenity::Error> for Error {
    fn from(value: serenity::Error) -> Self {
        Error::SerenityError(Box::new(value))
    }
}

impl Error {
    pub fn bot(s: &str) -> Self {
        Self::BotError(s.to_string())
    }
    pub fn auth(s: &str) -> Self {
        Self::TypeAuthdError(s.to_string())
    }
}
