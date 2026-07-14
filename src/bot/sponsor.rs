use std::sync::Arc;
use std::time::Duration;

use log::{debug, error, info};
use serenity::all::{Context, GuildId, RoleId, UserId};

use crate::services::BotDatabaseService;
use crate::utils::now_unix;

/// How often the sponsor watcher re-checks every stored sponsorship.
const CHECK_INTERVAL_SECS: u64 = 5 * 60;

/// Background loop that keeps sponsor roles in sync with the database.
///
/// Every [`CHECK_INTERVAL_SECS`] seconds it:
///  - revokes the role and deletes the record for expired sponsorships,
///  - re-issues the role to active sponsors that are missing it.
///
/// Each sponsorship record carries its own role, so several different
/// sponsor roles are managed independently.
pub async fn run_sponsor_loop(ctx: Context, db: Arc<BotDatabaseService>, guilds: Vec<GuildId>) {
    info!("Sponsor role watcher started. Interval: {CHECK_INTERVAL_SECS}s");

    let mut interval = tokio::time::interval(Duration::from_secs(CHECK_INTERVAL_SECS));

    loop {
        interval.tick().await;

        if let Err(e) = tick(&ctx, &db, &guilds).await {
            error!("Sponsor watcher tick failed: {e}");
        }
    }
}

async fn tick(
    ctx: &Context,
    db: &Arc<BotDatabaseService>,
    guilds: &[GuildId],
) -> Result<(), crate::error::Error> {
    let sponsorships = db.get_sponsorships().await?;
    let now = now_unix();

    for sponsorship in sponsorships {
        let user_id = match sponsorship.discord_id.parse::<u64>() {
            Ok(id) => UserId::new(id),
            Err(_) => {
                error!(
                    "Invalid discord id stored in sponsorships table: {}",
                    sponsorship.discord_id
                );
                continue;
            }
        };

        let role_id = match sponsorship.role_id.parse::<u64>() {
            Ok(id) => RoleId::new(id),
            Err(_) => {
                error!(
                    "Invalid role id stored in sponsorships table: {}",
                    sponsorship.role_id
                );
                continue;
            }
        };

        // Expired sponsorship -> revoke the role everywhere and drop the record.
        if let Some(expires_at) = sponsorship.expires_at {
            if expires_at <= now {
                debug!("Sponsorship {role_id} for {user_id} expired, revoking role.");

                for guild in guilds {
                    if let Err(e) = ctx
                        .http
                        .remove_member_role(*guild, user_id, role_id, Some("Sponsorship expired"))
                        .await
                    {
                        debug!("Couldn't remove sponsor role {role_id} from {user_id} in {guild}: {e}");
                    }
                }

                db.remove_sponsorship(&sponsorship.discord_id, &sponsorship.role_id)
                    .await?;
                continue;
            }
        }

        // Active sponsorship -> make sure the member still has the role.
        for guild in guilds {
            let member = match ctx.http.get_member(*guild, user_id).await {
                Ok(member) => member,
                // The user is most likely not a member of this guild, skip it.
                Err(_) => continue,
            };

            if member.roles.contains(&role_id) {
                continue;
            }

            debug!("Re-issuing sponsor role {role_id} to {user_id} in {guild}.");
            if let Err(e) = ctx
                .http
                .add_member_role(*guild, user_id, role_id, Some("Sponsorship active"))
                .await
            {
                error!("Failed to add sponsor role {role_id} to {user_id} in {guild}: {e}");
            }
        }
    }

    Ok(())
}
