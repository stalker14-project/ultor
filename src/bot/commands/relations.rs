use std::fmt::Write;
use std::sync::Arc;

use serenity::all::{
    CommandOptionType, CreateAllowedMentions, CreateAttachment, CreateCommandOption,
};

use super::*;
use crate::services::{
    FactionRelation, FactionRelations, FactionRelationsService, ServicesContainer,
};

#[derive(Debug)]
pub struct RelationsCommand {
    service: Arc<FactionRelationsService>,
}

impl RelationsCommand {
    pub fn new(services: &ServicesContainer) -> Self {
        Self {
            service: services.get_unsafe(),
        }
    }
}

#[async_trait]
impl DiscordCommandHandler for RelationsCommand {
    fn definition(&self) -> DiscordCommandDefinition {
        DiscordCommandDefinition::new_local("relations", true, false)
    }

    fn registration(&self) -> CreateCommand {
        CreateCommand::new("relations")
            .name_localized("ru", "отношения")
            .description("Show saved faction relations from the game database")
            .description_localized("ru", "Сохранённые отношения группировок из базы игры")
            .dm_permission(false)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "faction",
                    "Optional faction name, ID, or table number",
                )
                .description_localized("ru", "Название, ID или номер группировки из таблицы")
                .max_length(100),
            )
    }

    async fn handler(&self, opts: &[ResolvedOption]) -> DiscordCommandResponse {
        let snapshot = match self.service.snapshot().await {
            Ok(snapshot) => snapshot,
            Err(error) => {
                log::warn!("Could not fetch faction relations: {error}");
                return DiscordCommandResponse::followup_response(
                    "Faction relations are temporarily unavailable. Please try again shortly. / Отношения временно недоступны. Попробуйте чуть позже.", false);
            }
        };
        if snapshot.factions.is_empty() {
            return DiscordCommandResponse::followup_response(
                "No faction relations have been saved in the database. / В базе нет сохранённых отношений группировок.", false);
        }
        let faction = opts.iter().find_map(|opt| match (opt.name, &opt.value) {
            ("faction", ResolvedValue::String(value)) => Some(*value),
            _ => None,
        });
        let (title, text) = match faction {
            Some(query) => match find_faction(&snapshot, query) {
                Some(index) => (
                    format!("Отношения: {}", safe_name(&snapshot.factions[index]).chars().take(100).collect::<String>()),
                    faction_text(&snapshot, index),
                ),
                None => return DiscordCommandResponse::followup_response(
                    "Faction not found. Use /relations for the names and numbers. / Группировка не найдена: посмотрите названия и номера через /relations.", false),
            },
            None => ("Отношения группировок / Faction relations".to_owned(), matrix_text(&snapshot)),
        };

        let mut response =
            CreateInteractionResponseFollowup::new().allowed_mentions(CreateAllowedMentions::new());
        // Discord limits embed descriptions to 4096 characters. Keep a complete export
        // if the server adds enough factions to exceed that limit.
        let description = if text.encode_utf16().count() <= 4096 {
            text
        } else {
            response = response.add_file(CreateAttachment::bytes(
                text.into_bytes(),
                "faction-relations.txt",
            ));
            "Полная таблица во вложении. Для краткого просмотра: /relations faction:<название>."
                .to_owned()
        };
        DiscordCommandResponse::Followup(
            response.embed(
                CreateEmbed::new()
                    .title(title)
                    .description(description)
                    .color(Color::from_rgb(176, 143, 84))
                    .footer(CreateEmbedFooter::new(
                        "Сохранённые отношения • кэш до 10 сек. • Повторите команду для обновления",
                    )),
            ),
        )
    }
}

fn safe_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_control() || "`*_~|<>@\\".contains(c) {
                ' '
            } else {
                c
            }
        })
        .collect()
}

fn find_faction(snapshot: &FactionRelations, query: &str) -> Option<usize> {
    let query = query.trim().to_lowercase();
    if let Ok(number) = query.parse::<usize>() {
        return number
            .checked_sub(1)
            .filter(|&i| i < snapshot.factions.len());
    }
    snapshot
        .factions
        .iter()
        .position(|name| name.to_lowercase() == query)
}

fn matrix_text(snapshot: &FactionRelations) -> String {
    let width = snapshot.factions.len().to_string().len().max(2);
    let mut text = format!("```text\n{:width$}  ", "");
    for i in 1..=snapshot.factions.len() {
        let _ = write!(text, "{i:0width$} ");
    }
    text.push('\n');
    for i in 0..snapshot.factions.len() {
        let _ = write!(text, "{:0width$}  ", i + 1);
        for j in 0..snapshot.factions.len() {
            let symbol = if i == j {
                '-'
            } else {
                snapshot.relation(i, j).map_or('?', FactionRelation::symbol)
            };
            let _ = write!(text, "{symbol:>width$} ");
        }
        text.push('\n');
    }
    text.push_str("```\n🟩 A — Союз / Alliance\n🟨 N — Нейтралитет / Neutral\n🟧 H — Конфликт / Hostile\n🟥 W — Война / War\n? — Нет записи / Unknown\n- — Своя группировка / Same faction\n\n");
    for (i, faction) in snapshot.factions.iter().enumerate() {
        let _ = writeln!(text, "**{:0width$}** {}", i + 1, safe_name(faction));
    }
    text
}

fn faction_text(snapshot: &FactionRelations, index: usize) -> String {
    let mut text = String::new();
    for (relation, label) in [
        (Some(FactionRelation::Alliance), "🟩 Союз / Alliance"),
        (Some(FactionRelation::Neutral), "🟨 Нейтралитет / Neutral"),
        (Some(FactionRelation::Hostile), "🟧 Конфликт / Hostile"),
        (Some(FactionRelation::War), "🟥 Война / War"),
        (None, "❔ Нет записи / Unknown"),
    ] {
        let names: Vec<_> = snapshot
            .factions
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != index && snapshot.relation(index, *i) == relation)
            .map(|(_, faction)| safe_name(faction))
            .collect();
        let _ = writeln!(
            text,
            "**{label}**\n{}\n",
            if names.is_empty() {
                "—".to_owned()
            } else {
                names.join(", ")
            }
        );
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_matrix_and_resolves_raw_faction_names() {
        use crate::services::FactionRelationOverride;
        let snapshot = FactionRelations::from_database(vec![
            FactionRelationOverride {
                faction_a: "NewBand".into(),
                faction_b: "Свобода".into(),
                relation_type: 3,
            },
            FactionRelationOverride {
                faction_a: "Долг".into(),
                faction_b: "Свобода".into(),
                relation_type: 0,
            },
        ])
        .unwrap();
        let text = matrix_text(&snapshot);
        assert!(text.contains("01 02"));
        assert!(text.contains("01   -  ?  W"));
        assert!(text.contains("03   W  N  -"));
        assert!(text.contains("**01** NewBand"));
        assert_eq!(find_faction(&snapshot, " СВОБОДА "), Some(2));
        assert_eq!(find_faction(&snapshot, "newband"), Some(0));
        assert_eq!(find_faction(&snapshot, "02"), Some(1));
        assert_eq!(find_faction(&snapshot, "0"), None);
        assert_eq!(find_faction(&snapshot, "4"), None);
        assert!(faction_text(&snapshot, 0).contains("**🟥 Война / War**\nСвобода"));
        assert!(faction_text(&snapshot, 0).contains("**❔ Нет записи / Unknown**\nДолг"));
        assert!(!faction_text(&snapshot, 0).contains("NewBand"));
    }

    #[test]
    fn current_sized_matrix_fits_discord_and_names_cannot_break_code_blocks() {
        use crate::services::FactionRelationOverride;
        let snapshot = FactionRelations::from_database(
            (2..=16)
                .map(|i| FactionRelationOverride {
                    faction_a: "Группировка 1".into(),
                    faction_b: format!("Группировка {i}"),
                    relation_type: 0,
                })
                .collect(),
        )
        .unwrap();
        assert!(matrix_text(&snapshot).encode_utf16().count() <= 4096);
        assert!(!safe_name("```\n@everyone").contains('`'));
        assert!(!safe_name("```\n@everyone").contains('@'));
    }
}
