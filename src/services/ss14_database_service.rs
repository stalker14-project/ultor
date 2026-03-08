use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug)]
pub struct SS14DatabaseService {
    inner: PgPool,
}

impl SS14DatabaseService {
    pub fn new(pg_url: String) -> Result<Self, crate::error::Error> {
        let pg_pool = PgPool::connect_lazy(pg_url.as_str())?;

        Ok(Self { inner: pg_pool })
    }

    pub async fn get_login(&self, user_id: Uuid) -> Result<Option<String>, crate::error::Error> {
        let row = sqlx::query("SELECT last_seen_user_name FROM player WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&self.inner)
            .await;

        if let Err(sqlx::error::Error::RowNotFound) = row {
            return Ok(None);
        }

        let row = row?;
        let user_id: String = row.get(0);
        Ok(Some(user_id))
    }

    pub async fn delete_pref(&self, user_id: Uuid) -> Result<(), crate::error::Error> {
        let mut tx = self.inner.begin().await?;
        let affected = sqlx::query(
            "DELETE FROM preference WHERE user_id = $1"
        )
        .bind(user_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        log::debug!("Deleted {affected} for {user_id}");
        Ok(())
    }
}
