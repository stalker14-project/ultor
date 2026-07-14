use rand::RngExt;
use serenity::all::Color;

use crate::services::SS14AuthClientService;

pub const RED_COLOR: Color = Color::from_rgb(255, 0, 0);

pub fn gen_random_color() -> Color {
    let mut rng = rand::rng();
    Color::from_rgb(rng.random(), rng.random(), rng.random())
}

/// Reads the list of configured sponsor roles from `discord.sponsor_roles`.
///
/// Each entry is a `(name, role_id)` pair. Malformed entries are skipped.
pub fn sponsor_roles() -> Vec<(String, u64)> {
    let Some(config) = crate::CONFIG.get() else {
        return Vec::new();
    };

    let Some(array) = config
        .get_path("discord.sponsor_roles")
        .and_then(|v| v.as_array())
    else {
        return Vec::new();
    };

    array
        .iter()
        .filter_map(|item| {
            let obj = item.as_object()?;
            let name = obj.get("name")?.as_str()?.to_string();
            let id = obj.get("id")?.as_str()?.parse::<u64>().ok()?;
            Some((name, id))
        })
        .collect()
}

/// Current unix timestamp in seconds.
pub fn now_unix() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Parses a human duration expression into a number of seconds.
///
/// Supported units: `s` (seconds), `m` (minutes), `h` (hours), `d` (days),
/// `w` (weeks). Multiple segments may be combined, e.g. `1w3d12h`.
/// Every numeric part must be followed by a unit, otherwise `None` is returned.
pub fn parse_duration_secs(input: &str) -> Option<i64> {
    let mut total: i64 = 0;
    let mut num = String::new();
    let mut seen = false;

    for c in input.chars() {
        if c.is_ascii_digit() {
            num.push(c);
            continue;
        }

        if num.is_empty() {
            return None;
        }

        let value: i64 = num.parse().ok()?;
        let unit = match c.to_ascii_lowercase() {
            's' => 1,
            'm' => 60,
            'h' => 3600,
            'd' => 86_400,
            'w' => 604_800,
            _ => return None,
        };

        total = total.checked_add(value.checked_mul(unit)?)?;
        num.clear();
        seen = true;
    }

    // A trailing number without a unit is invalid.
    if !num.is_empty() || !seen {
        return None;
    }

    Some(total)
}

#[macro_export]
macro_rules! try_discord_unwrap {
    // Pattern: Option<T>
    ($opt:expr, none => $none_msg:expr, $(ephemeral => $ephemeral:expr)? ) => {{
        match $opt {
            Some(v) => v,
            None => return $crate::bot::commands::DiscordCommandResponse::followup_embed_response(
                $none_msg,
                None,
                Some($crate::utils::RED_COLOR),
                try_discord_unwrap!(@ephemeral $($ephemeral)?),
            ),
        }
    }};

    // Pattern: Result<T, E>
    ($res:expr, error => $err_msg:expr, log => $log_msg:expr, $(ephemeral => $ephemeral:expr)? ) => {{
        match $res {
            Ok(v) => v,
            Err(e) => {
                let err_id = uuid::Uuid::new_v4();
                log::error!("{}. {}. Error: {}", err_id, $log_msg, e);
                return $crate::bot::commands::DiscordCommandResponse::followup_embed_response(
                    $err_msg,
                    Some(&err_id.to_string()),
                    Some($crate::utils::RED_COLOR),
                    try_discord_unwrap!(@ephemeral $($ephemeral)?),
                );
            }
        }
    }};

    // Pattern: Result<Option<T>, E>
    ($resopt:expr, none => $none_msg:expr, error => $err_msg:expr, log => $log_msg:expr, $(ephemeral => $ephemeral:expr)? ) => {{
        match $resopt {
            Ok(Some(v)) => v,
            Ok(None) => return $crate::bot::commands::DiscordCommandResponse::followup_embed_response(
                $none_msg,
                None,
                Some($crate::utils::RED_COLOR),
                try_discord_unwrap!(@ephemeral $($ephemeral)?),
            ),
            Err(e) => {
                let err_id = uuid::Uuid::new_v4();
                log::error!("{}. {}. Error: {}", err_id, $log_msg, e);
                return $crate::bot::commands::DiscordCommandResponse::followup_embed_response(
                    $err_msg,
                    Some(&err_id.to_string()),
                    Some($crate::utils::RED_COLOR),
                    try_discord_unwrap!(@ephemeral $($ephemeral)?),
                );
            }
        }
    }};


    (@ephemeral $e:expr) => { $e };
    (@ephemeral) => { true };
}

#[macro_export]
macro_rules! extract_discord_arg {
    // ResolvedValue::String
    (
        $opts:expr,
        $name:literal,
        String
    ) => {
        $opts.iter().find_map(|opt| match (opt.name, &opt.value) {
            ($name, serenity::all::ResolvedValue::String(i)) => Some(i.to_string()),
            _ => None,
        })
    };

    // ResolvedValue::*
    (
        $opts:expr,
        $name:literal,
        $dtype:ident
    ) => {
        $opts.iter().find_map(|opt| match (opt.name, &opt.value) {
            ($name, serenity::all::ResolvedValue::$dtype(i)) => Some(i),
            _ => None,
        })
    };
}

pub async fn format_extra_data(
    discord_id: &str,
    ss14_client: &std::sync::Arc<SS14AuthClientService>,
) -> Result<String, crate::error::Error> {
    use serde_json::Value;

    let capitalize_key = |key: &str| {
        key.split('_')
            .map(|part| {
                let mut c = part.chars();
                match c.next() {
                    Some(first) => first.to_uppercase().chain(c).collect(),
                    None => String::new(),
                }
            })
            .collect::<Vec<String>>()
            .join(" ")
    };

    let value = ss14_client.get_extra_data(discord_id.to_string()).await?;

    match value {
        Some(value) => {
            let obj = value.as_object().unwrap();
            let mut result = String::new();

            for (k, v) in obj {
                match v {
                    Value::String(s) => {
                        result.push_str(&format!("{}: {}\n", capitalize_key(k), s));
                    }
                    Value::Number(n) if n.is_i64() || n.is_u64() => {
                        result.push_str(&format!("{}: {}\n", capitalize_key(k), n));
                    }
                    _ => {}
                }
            }

            if result.is_empty() {
                Ok("No extra data found.".to_string())
            } else {
                Ok(result)
            }
        }
        None => Ok("No extra data found.".to_string()),
    }
}
