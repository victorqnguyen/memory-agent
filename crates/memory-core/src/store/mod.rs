pub mod dedup;
pub mod eventlog;
pub mod memory;
pub mod metrics;
pub mod privacy;
pub mod relations;
pub mod schema;
pub mod scope;

use rusqlite::Connection;

use crate::config::Config;
use crate::Error;

pub struct Store {
    conn: Connection,
    config: Config,
}

impl Store {
    pub fn open(path: &str, config: Config, passphrase: Option<&str>) -> crate::Result<Self> {
        if config.storage.encryption_enabled && passphrase.is_none() {
            return Err(Error::Encryption(
                "encryption is enabled but no passphrase was provided".into(),
            ));
        }
        let mut conn = Connection::open(path)?;
        if let Some(key) = passphrase {
            apply_passphrase(&conn, key)?;
            verify_access(&conn)?;
        }
        configure_connection(&conn, &config)?;
        schema::check_version(&conn)?;
        run_migrations(&mut conn)?;
        Ok(Self { conn, config })
    }

    pub fn open_in_memory() -> crate::Result<Self> {
        Self::open_in_memory_with_config(Config::default())
    }

    pub fn open_in_memory_with_config(config: Config) -> crate::Result<Self> {
        let mut conn = Connection::open_in_memory()?;
        configure_connection(&conn, &config)?;
        run_migrations(&mut conn)?;
        Ok(Self { conn, config })
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn get_metadata(&self, key: &str) -> crate::Result<Option<String>> {
        match self.conn.query_row(
            "SELECT value FROM _metadata WHERE key = ?1",
            rusqlite::params![key],
            |row| row.get::<_, String>(0),
        ) {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_metadata(&self, key: &str, value: &str) -> crate::Result<()> {
        self.conn.execute(
            "INSERT INTO _metadata (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }

    pub fn schema_version(&self) -> crate::Result<i64> {
        Ok(self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))?)
    }

    /// Check if a database file is encrypted (requires `encryption` feature to detect).
    pub fn is_encrypted(path: &str) -> bool {
        // Try opening without a key — if it fails on sqlite_master, it's encrypted
        let conn = match Connection::open(path) {
            Ok(c) => c,
            Err(_) => return false,
        };
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
            .is_err()
    }

    /// Encrypt an existing unencrypted database using SQLCipher's ATTACH + sqlcipher_export.
    /// Uses backup-before-rename: original DB is preserved until verification passes.
    #[cfg(feature = "encryption")]
    pub fn encrypt(path: &str, passphrase: &str, config: Config) -> crate::Result<()> {
        use std::fs;

        if Self::is_encrypted(path) {
            return Err(Error::Encryption(
                "database is already encrypted — decrypt first or delete and reinitialise".into(),
            ));
        }

        let tmp_path = format!("{}.encrypting", path);
        let backup_path = format!("{}.backup", path);

        // Remove stale temp file from a previous failed attempt
        fs::remove_file(&tmp_path).ok();

        // Export to encrypted temp file
        {
            let conn = Connection::open(path)?;
            conn.execute_batch(&format!(
                "ATTACH DATABASE '{}' AS encrypted KEY '{}';",
                escape_sql_string(&tmp_path),
                escape_sql_string(passphrase)
            ))?;
            conn.execute_batch("SELECT sqlcipher_export('encrypted');")?;
            let ver: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
            conn.execute_batch(&format!("PRAGMA encrypted.user_version = {};", ver))?;
            conn.execute_batch("DETACH DATABASE encrypted;")?;
        }

        // Verify the encrypted file BEFORE touching the original
        let verify_result = Store::open(&tmp_path, config.clone(), Some(passphrase));
        if let Err(e) = verify_result {
            fs::remove_file(&tmp_path).ok();
            return Err(Error::Encryption(format!(
                "encrypted DB failed verification, original preserved: {}",
                e
            )));
        }
        drop(verify_result);

        // Backup original, then replace
        fs::rename(path, &backup_path)
            .map_err(|e| Error::Encryption(format!("failed to backup original: {}", e)))?;
        cleanup_wal_files(path);

        if let Err(e) = fs::rename(&tmp_path, path) {
            // Restore backup on failure
            fs::rename(&backup_path, path).ok();
            return Err(Error::Encryption(format!("failed to replace db: {}", e)));
        }

        // Success — remove backup
        fs::remove_file(&backup_path).ok();
        Ok(())
    }

    /// Rotate the encryption passphrase in-place using SQLCipher's PRAGMA rekey.
    /// No intermediate plaintext file is created — the re-encryption is atomic.
    #[cfg(feature = "encryption")]
    pub fn rekey(&self, new_passphrase: &str) -> crate::Result<()> {
        self.conn
            .pragma_update(None, "rekey", new_passphrase)
            .map_err(|e| Error::Encryption(format!("rekey failed: {}", e)))?;
        // Verify the new passphrase works by reading user_version
        self.conn
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i32>(0))
            .map_err(|_| {
                Error::Encryption("rekey verification failed — database may be in an inconsistent state".into())
            })?;
        Ok(())
    }

    /// Decrypt an encrypted database back to plaintext.
    /// Uses backup-before-rename: original DB is preserved until verification passes.
    #[cfg(feature = "encryption")]
    pub fn decrypt(path: &str, passphrase: &str, config: Config) -> crate::Result<()> {
        use std::fs;
        let tmp_path = format!("{}.decrypting", path);
        let backup_path = format!("{}.backup", path);

        // Export to plaintext temp file
        {
            let conn = Connection::open(path)?;
            apply_passphrase(&conn, passphrase)?;
            verify_access(&conn)?;
            conn.execute_batch(&format!(
                "ATTACH DATABASE '{}' AS plaintext KEY '';",
                escape_sql_string(&tmp_path)
            ))?;
            conn.execute_batch("SELECT sqlcipher_export('plaintext');")?;
            let ver: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
            conn.execute_batch(&format!("PRAGMA plaintext.user_version = {};", ver))?;
            conn.execute_batch("DETACH DATABASE plaintext;")?;
        }

        // Verify the plaintext file BEFORE touching the original
        let mut verify_config = config;
        verify_config.storage.encryption_enabled = false;
        let verify_result = Store::open(&tmp_path, verify_config.clone(), None);
        if let Err(e) = verify_result {
            fs::remove_file(&tmp_path).ok();
            return Err(Error::Encryption(format!(
                "decrypted DB failed verification, original preserved: {}",
                e
            )));
        }
        drop(verify_result);

        // Backup original, then replace
        fs::rename(path, &backup_path)
            .map_err(|e| Error::Encryption(format!("failed to backup original: {}", e)))?;
        cleanup_wal_files(path);

        if let Err(e) = fs::rename(&tmp_path, path) {
            fs::rename(&backup_path, path).ok();
            return Err(Error::Encryption(format!("failed to replace db: {}", e)));
        }

        // Success — remove backup
        fs::remove_file(&backup_path).ok();
        Ok(())
    }
}

fn apply_passphrase(conn: &Connection, passphrase: &str) -> crate::Result<()> {
    // PRAGMA key must be the FIRST statement after opening (SQLCipher requirement)
    conn.pragma_update(None, "key", passphrase)
        .map_err(|e| Error::Encryption(format!("failed to set encryption key: {}", e)))?;
    Ok(())
}

fn verify_access(conn: &Connection) -> crate::Result<()> {
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |_| Ok(()))
        .map_err(|_| {
            Error::Encryption("cannot access database — wrong passphrase or not encrypted".into())
        })?;
    Ok(())
}

fn configure_connection(conn: &Connection, config: &Config) -> rusqlite::Result<()> {
    conn.execute_batch(&format!(
        "PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = {};
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA cache_size = -{};",
        config.storage.busy_timeout_ms, config.storage.cache_size_kb
    ))?;
    Ok(())
}

fn run_migrations(conn: &mut Connection) -> crate::Result<()> {
    let migrations = schema::migrations();
    migrations
        .to_latest(conn)
        .map_err(|e| Error::Migration(e.to_string()))?;
    Ok(())
}

#[cfg(feature = "encryption")]
fn escape_sql_string(s: &str) -> String {
    s.replace('\'', "''")
}

/// Remove stale WAL-mode sidecar files after replacing a DB file.
/// SQLite creates .db-shm and .db-wal; these are invalid after the main
/// file is swapped and will confuse the new connection.
#[cfg(feature = "encryption")]
fn cleanup_wal_files(path: &str) {
    use std::fs;
    fs::remove_file(format!("{}-shm", path)).ok();
    fs::remove_file(format!("{}-wal", path)).ok();
}
