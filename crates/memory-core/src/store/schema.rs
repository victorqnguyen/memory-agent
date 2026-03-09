use rusqlite_migration::{Migrations, M};

use crate::Error;

pub fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(include_str!("migrations/001_initial.sql")),
        M::up(include_str!("migrations/002_metrics.sql")),
        M::up(include_str!("migrations/003_relations.sql")),
        M::up(include_str!("migrations/004_cascade_fks.sql")),
        M::up(include_str!("migrations/005_cleanup_orphans.sql")),
        M::up(include_str!("migrations/006_activity_tokens.sql")),
        M::up(include_str!("migrations/007_fix_fts_update_trigger.sql")),
        M::up(include_str!("migrations/008_supersedes_index.sql")),
        M::up(include_str!(
            "migrations/009_fix_fts_hard_delete_trigger.sql"
        )),
        M::up(include_str!("migrations/010_event_log.sql")),
    ])
}

pub const SCHEMA_VERSION: i64 = 10;

pub fn check_version(conn: &rusqlite::Connection) -> crate::Result<()> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if current > SCHEMA_VERSION {
        return Err(Error::SchemaVersionTooNew {
            found: current,
            supported: SCHEMA_VERSION,
        });
    }
    Ok(())
}
