use crate::error::Error;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::PathBuf;

/// A single sponsorship record stored in the bot database.
///
/// A user may hold several sponsorships at once, one per role.
#[derive(Debug, Clone)]
pub struct Sponsorship {
    pub discord_id: String,
    pub role_id: String,
    /// Unix timestamp (seconds) at which the sponsorship expires.
    /// `None` means the sponsorship is permanent.
    pub expires_at: Option<i64>,
}

#[derive(Debug)]
pub struct BotDatabaseService {
    inner: SqlitePool,
}

impl BotDatabaseService {
    pub async fn new(database_path: String, migrations_path: String) -> Result<Self, Error> {
        let database_path = PathBuf::from(database_path);
        let migrations_path = PathBuf::from(migrations_path);

        let options = SqliteConnectOptions::new()
            .create_if_missing(true)
            .filename(database_path);

        let pool = SqlitePoolOptions::new().connect_lazy_with(options);

        let migrator = Migrator::new(migrations_path).await?;
        migrator.run(&pool).await?;

        Ok(Self { inner: pool })
    }

    // implement your own methods here
    // if you want to modify database structure -> look at migrations directory at the root of the project

    /// Inserts a new sponsorship or updates the expiration of an existing
    /// user/role pair.
    pub async fn upsert_sponsorship(
        &self,
        discord_id: &str,
        role_id: &str,
        expires_at: Option<i64>,
        created_at: i64,
    ) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO sponsorships (discord_id, role_id, expires_at, created_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(discord_id, role_id) DO UPDATE SET expires_at = ?3",
        )
        .bind(discord_id)
        .bind(role_id)
        .bind(expires_at)
        .bind(created_at)
        .execute(&self.inner)
        .await?;

        Ok(())
    }

    /// Returns all currently stored sponsorships.
    pub async fn get_sponsorships(&self) -> Result<Vec<Sponsorship>, Error> {
        let rows = sqlx::query("SELECT discord_id, role_id, expires_at FROM sponsorships")
            .fetch_all(&self.inner)
            .await?;

        let sponsorships = rows
            .into_iter()
            .map(|row| Sponsorship {
                discord_id: row.get(0),
                role_id: row.get(1),
                expires_at: row.get(2),
            })
            .collect();

        Ok(sponsorships)
    }

    /// Removes the sponsorship record for the given user/role pair.
    pub async fn remove_sponsorship(&self, discord_id: &str, role_id: &str) -> Result<(), Error> {
        sqlx::query("DELETE FROM sponsorships WHERE discord_id = ?1 AND role_id = ?2")
            .bind(discord_id)
            .bind(role_id)
            .execute(&self.inner)
            .await?;

        Ok(())
    }
}
