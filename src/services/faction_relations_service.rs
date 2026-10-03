use std::collections::{BTreeSet, HashMap};
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use super::{FactionRelationOverride, SS14DatabaseService};
use crate::Error;

const CACHE_TTL: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FactionRelation {
    Neutral,
    Alliance,
    Hostile,
    War,
}

impl FactionRelation {
    pub fn symbol(self) -> char {
        match self {
            Self::Alliance => 'A',
            Self::Neutral => 'N',
            Self::Hostile => 'H',
            Self::War => 'W',
        }
    }

    fn from_database(value: i32) -> Result<Self, Error> {
        // Content.Shared/_Stalker_EN/FactionRelations/STFactionRelationType.cs
        match value {
            0 => Ok(Self::Neutral),
            1 => Ok(Self::Alliance),
            2 => Ok(Self::Hostile),
            3 => Ok(Self::War),
            _ => Err(Error::bot("Unknown faction relation type in database")),
        }
    }
}

#[derive(Debug)]
pub struct FactionRelations {
    /// Raw names discovered from both columns of the DB query, sorted for display.
    pub factions: Vec<String>,
    relations: HashMap<(usize, usize), FactionRelation>,
}

impl FactionRelations {
    pub(crate) fn from_database(rows: Vec<FactionRelationOverride>) -> Result<Self, Error> {
        let factions: Vec<_> = rows
            .iter()
            .flat_map(|row| [&row.faction_a, &row.faction_b])
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let positions: HashMap<_, _> = factions
            .iter()
            .enumerate()
            .map(|(i, name)| (name.as_str(), i))
            .collect();
        let mut relations = HashMap::new();
        for row in rows {
            let a = positions[row.faction_a.as_str()];
            let b = positions[row.faction_b.as_str()];
            if a == b {
                continue;
            }
            let relation = FactionRelation::from_database(row.relation_type)?;
            if relations.insert((a.min(b), a.max(b)), relation).is_some() {
                return Err(Error::bot("Duplicate faction pair in database"));
            }
        }
        Ok(Self {
            factions,
            relations,
        })
    }

    /// None means there is no saved relation, not that the factions are neutral.
    pub fn relation(&self, a: usize, b: usize) -> Option<FactionRelation> {
        self.relations.get(&(a.min(b), a.max(b))).copied()
    }
}

#[derive(Debug)]
struct CachedSnapshot {
    fetched_at: Instant,
    snapshot: Option<Arc<FactionRelations>>,
}

#[derive(Debug)]
pub struct FactionRelationsService {
    database: Arc<SS14DatabaseService>,
    cache: Mutex<Option<CachedSnapshot>>,
}

impl FactionRelationsService {
    pub fn new(database: Arc<SS14DatabaseService>) -> Self {
        Self {
            database,
            cache: Mutex::new(None),
        }
    }

    pub async fn snapshot(&self) -> Result<Arc<FactionRelations>, Error> {
        self.snapshot_with(self.database.faction_relation_overrides())
            .await
    }

    async fn snapshot_with(
        &self,
        fetch: impl Future<Output = Result<Vec<FactionRelationOverride>, Error>>,
    ) -> Result<Arc<FactionRelations>, Error> {
        // Concurrent commands share one query. Failed queries are cached briefly too.
        let mut cache = self.cache.lock().await;
        if let Some(cached) = cache
            .as_ref()
            .filter(|c| c.fetched_at.elapsed() < CACHE_TTL)
        {
            return cached.snapshot.clone().ok_or_else(|| {
                Error::bot("Faction relations database is temporarily unavailable")
            });
        }
        let fetched_at = Instant::now();
        let result = match tokio::time::timeout(Duration::from_secs(8), fetch).await {
            Ok(Ok(rows)) => FactionRelations::from_database(rows).map(Arc::new),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(Error::bot("Faction relations database query timed out")),
        };
        *cache = Some(CachedSnapshot {
            fetched_at,
            snapshot: result.as_ref().ok().cloned(),
        });
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn pair(a: &str, b: &str, relation_type: i32) -> FactionRelationOverride {
        FactionRelationOverride {
            faction_a: a.to_owned(),
            faction_b: b.to_owned(),
            relation_type,
        }
    }

    #[test]
    fn discovers_raw_names_in_both_columns_and_keeps_missing_pairs_unknown() {
        let snapshot = FactionRelations::from_database(vec![
            pair("NewBand", "Стая", 3),
            pair("NewBand", "AnotherBand", 0),
            pair("UN", "Стая", 1),
        ])
        .unwrap();
        assert_eq!(snapshot.factions, ["AnotherBand", "NewBand", "UN", "Стая"]);
        assert_eq!(snapshot.relation(0, 1), Some(FactionRelation::Neutral));
        assert_eq!(snapshot.relation(1, 3), Some(FactionRelation::War));
        assert_eq!(snapshot.relation(3, 1), Some(FactionRelation::War));
        assert_eq!(snapshot.relation(2, 3), Some(FactionRelation::Alliance));
        assert_eq!(snapshot.relation(0, 3), None);
        assert_eq!(snapshot.relation(0, 0), None);
        assert!(FactionRelations::from_database(vec![])
            .unwrap()
            .factions
            .is_empty());
    }

    #[test]
    fn rejects_corrupt_relations_instead_of_guessing() {
        assert!(FactionRelations::from_database(vec![pair("A", "B", 99)]).is_err());
        assert!(
            FactionRelations::from_database(vec![pair("A", "B", 0), pair("B", "A", 3)]).is_err()
        );
    }

    #[tokio::test]
    async fn shares_queries_discovers_new_bands_and_refreshes_resets_without_stale_data() {
        // Lazy pool is never contacted: injected query results exercise the cache.
        let database = Arc::new(
            SS14DatabaseService::new("postgres://unused:unused@localhost/unused".to_owned())
                .unwrap(),
        );
        let service = FactionRelationsService::new(database);
        let queries = AtomicUsize::new(0);
        let fetch = || async {
            queries.fetch_add(1, Ordering::SeqCst);
            tokio::task::yield_now().await;
            Ok(vec![pair("Bandits", "Loners", 3)])
        };
        let (a, b) = tokio::join!(
            service.snapshot_with(fetch()),
            service.snapshot_with(fetch())
        );
        let a = a.unwrap();
        assert!(Arc::ptr_eq(&a, &b.unwrap()));
        assert_eq!(queries.load(Ordering::SeqCst), 1);
        assert_eq!(a.relation(0, 1), Some(FactionRelation::War));

        service.cache.lock().await.as_mut().unwrap().fetched_at =
            Instant::now() - Duration::from_secs(11);
        let changed = service
            .snapshot_with(async { Ok(vec![pair("Loners", "NewBand", 1)]) })
            .await
            .unwrap();
        assert_eq!(changed.factions, ["Loners", "NewBand"]);
        assert_eq!(changed.relation(0, 1), Some(FactionRelation::Alliance));

        service.cache.lock().await.as_mut().unwrap().fetched_at =
            Instant::now() - Duration::from_secs(11);
        let reset = service.snapshot_with(async { Ok(vec![]) }).await.unwrap();
        assert!(reset.factions.is_empty());

        service.cache.lock().await.as_mut().unwrap().fetched_at =
            Instant::now() - Duration::from_secs(11);
        assert!(service
            .snapshot_with(async { Err(Error::bot("DB down")) })
            .await
            .is_err());
        assert!(service.snapshot_with(fetch()).await.is_err());
        assert_eq!(queries.load(Ordering::SeqCst), 1);
        assert!(service
            .cache
            .lock()
            .await
            .as_ref()
            .unwrap()
            .snapshot
            .is_none());
    }
}
