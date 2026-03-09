use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::store::Store;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    DerivedFrom,
    Supersedes,
    ConflictsWith,
    RelatedTo,
}

impl std::fmt::Display for RelationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DerivedFrom => write!(f, "derived_from"),
            Self::Supersedes => write!(f, "supersedes"),
            Self::ConflictsWith => write!(f, "conflicts_with"),
            Self::RelatedTo => write!(f, "related_to"),
        }
    }
}

impl std::str::FromStr for RelationType {
    type Err = Error;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "derived_from" => Ok(Self::DerivedFrom),
            "supersedes" => Ok(Self::Supersedes),
            "conflicts_with" => Ok(Self::ConflictsWith),
            "related_to" => Ok(Self::RelatedTo),
            other => Err(Error::InvalidInput(format!(
                "unknown relation type: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Relation {
    pub id: i64,
    pub source_id: i64,
    pub target_id: i64,
    pub relation_type: RelationType,
    pub created_at: String,
}

impl Store {
    pub fn add_relation(
        &self,
        source_id: i64,
        target_id: i64,
        rel_type: RelationType,
    ) -> Result<i64> {
        if source_id == target_id {
            return Err(Error::InvalidInput(
                "cannot create self-referential relation".to_string(),
            ));
        }
        // Verify both memories exist
        self.get(source_id)?;
        self.get(target_id)?;

        self.conn().execute(
            "INSERT OR IGNORE INTO relations (source_id, target_id, relation_type) VALUES (?1, ?2, ?3)",
            params![source_id, target_id, rel_type.to_string()],
        )?;
        Ok(self.conn().last_insert_rowid())
    }

    pub fn get_relations(&self, memory_id: i64) -> Result<Vec<Relation>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, source_id, target_id, relation_type, created_at
             FROM relations WHERE source_id = ?1 OR target_id = ?1",
        )?;
        let results = stmt
            .query_map(params![memory_id], |row| {
                let rt: String = row.get(3)?;
                Ok(Relation {
                    id: row.get(0)?,
                    source_id: row.get(1)?,
                    target_id: row.get(2)?,
                    relation_type: rt.parse().unwrap_or(RelationType::RelatedTo),
                    created_at: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(results)
    }

    pub fn superseded_ids(&self) -> Result<std::collections::HashSet<i64>> {
        let mut stmt = self.conn().prepare(
            "SELECT DISTINCT target_id FROM relations WHERE relation_type = 'supersedes'",
        )?;
        let ids: Vec<i64> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(ids.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_memory(store: &Store, key: &str, value: &str) -> i64 {
        use crate::types::SaveParams;
        store
            .save(SaveParams {
                key: key.to_string(),
                value: value.to_string(),
                scope: Some("/test".to_string()),
                source_type: Some(crate::types::SourceType::Explicit),
                tags: None,
                source_ref: None,
                source_commit: None,
            })
            .unwrap()
            .id()
    }

    #[test]
    fn add_and_get_relation() {
        let store = Store::open_in_memory().unwrap();
        let id_a = make_memory(&store, "mem/a", "value a");
        let id_b = make_memory(&store, "mem/b", "value b");

        let rel_id = store
            .add_relation(id_a, id_b, RelationType::RelatedTo)
            .unwrap();
        assert!(rel_id > 0);

        let relations = store.get_relations(id_a).unwrap();
        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].source_id, id_a);
        assert_eq!(relations[0].target_id, id_b);
        assert_eq!(relations[0].relation_type, RelationType::RelatedTo);
    }

    #[test]
    fn get_relations_returns_both_directions() {
        let store = Store::open_in_memory().unwrap();
        let id_a = make_memory(&store, "mem/a", "value a");
        let id_b = make_memory(&store, "mem/b", "value b");
        let id_c = make_memory(&store, "mem/c", "value c");

        store
            .add_relation(id_a, id_b, RelationType::DerivedFrom)
            .unwrap();
        store
            .add_relation(id_c, id_a, RelationType::Supersedes)
            .unwrap();

        let relations = store.get_relations(id_a).unwrap();
        assert_eq!(relations.len(), 2);
    }

    #[test]
    fn self_referential_relation_rejected() {
        let store = Store::open_in_memory().unwrap();
        let id_a = make_memory(&store, "mem/a", "value a");

        let err = store
            .add_relation(id_a, id_a, RelationType::RelatedTo)
            .unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
    }

    #[test]
    fn duplicate_relation_is_ignored() {
        let store = Store::open_in_memory().unwrap();
        let id_a = make_memory(&store, "mem/a", "value a");
        let id_b = make_memory(&store, "mem/b", "value b");

        store
            .add_relation(id_a, id_b, RelationType::RelatedTo)
            .unwrap();
        // Second insert with INSERT OR IGNORE returns 0 for last_insert_rowid
        store
            .add_relation(id_a, id_b, RelationType::RelatedTo)
            .unwrap();

        let relations = store.get_relations(id_a).unwrap();
        assert_eq!(relations.len(), 1);
    }

    #[test]
    fn superseded_ids_returns_correct_set() {
        let store = Store::open_in_memory().unwrap();
        let id_a = make_memory(&store, "mem/a", "value a");
        let id_b = make_memory(&store, "mem/b", "value b");
        let id_c = make_memory(&store, "mem/c", "value c");

        store
            .add_relation(id_a, id_b, RelationType::Supersedes)
            .unwrap();
        store
            .add_relation(id_a, id_c, RelationType::Supersedes)
            .unwrap();

        let superseded = store.superseded_ids().unwrap();
        assert!(superseded.contains(&id_b));
        assert!(superseded.contains(&id_c));
        assert!(!superseded.contains(&id_a));
    }

    #[test]
    fn relation_type_roundtrip() {
        for (s, expected) in [
            ("derived_from", RelationType::DerivedFrom),
            ("supersedes", RelationType::Supersedes),
            ("conflicts_with", RelationType::ConflictsWith),
            ("related_to", RelationType::RelatedTo),
        ] {
            let parsed: RelationType = s.parse().unwrap();
            assert_eq!(parsed, expected);
            assert_eq!(parsed.to_string(), s);
        }
        assert!("unknown".parse::<RelationType>().is_err());
    }
}
