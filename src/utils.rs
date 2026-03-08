use rand::RngExt;
use serenity::all::Color;

use crate::services::SS14AuthClientService;

pub const RED_COLOR: Color = Color::from_rgb(255, 0, 0);

pub fn gen_random_color() -> Color {
    let mut rng = rand::rng();
    Color::from_rgb(rng.random(), rng.random(), rng.random())
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
