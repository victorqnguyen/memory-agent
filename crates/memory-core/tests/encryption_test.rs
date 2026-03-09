#![cfg(feature = "encryption")]

use memory_core::types::{SaveParams, SearchParams};
use memory_core::{Config, Error, Store};

fn tmp_db() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db").to_str().unwrap().to_string();
    (dir, path)
}

fn search_query(query: &str) -> SearchParams {
    SearchParams {
        query: query.into(),
        scope: None,
        source_type: None,
        limit: None,
    }
}

#[test]
fn test_open_encrypted_db() {
    let (_dir, path) = tmp_db();
    let passphrase = "test-passphrase-123";

    // Create encrypted DB
    {
        let mut config = Config::default();
        config.storage.encryption_enabled = true;
        let store = Store::open(&path, config, Some(passphrase)).unwrap();
        store
            .save(SaveParams {
                key: "test/key".into(),
                value: "encrypted value".into(),
                scope: Some("/".into()),
                ..Default::default()
            })
            .unwrap();
    }

    // Reopen with same passphrase
    {
        let mut config = Config::default();
        config.storage.encryption_enabled = true;
        let store = Store::open(&path, config, Some(passphrase)).unwrap();
        let results = store.search(search_query("encrypted")).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].value_preview.contains("encrypted value"));
    }
}

#[test]
fn test_wrong_passphrase() {
    let (_dir, path) = tmp_db();
    let mut config = Config::default();
    config.storage.encryption_enabled = true;

    // Create with one passphrase
    {
        let store = Store::open(&path, config.clone(), Some("correct")).unwrap();
        store
            .save(SaveParams {
                key: "test/key".into(),
                value: "secret".into(),
                scope: Some("/".into()),
                ..Default::default()
            })
            .unwrap();
    }

    // Try opening with wrong passphrase
    match Store::open(&path, config, Some("wrong")) {
        Err(Error::Encryption(_)) => {}
        Err(other) => panic!("expected Encryption error, got: {:?}", other),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn test_encryption_enabled_no_passphrase() {
    let (_dir, path) = tmp_db();
    let mut config = Config::default();
    config.storage.encryption_enabled = true;

    match Store::open(&path, config, None) {
        Err(Error::Encryption(msg)) => assert!(msg.contains("no passphrase"), "msg: {}", msg),
        Err(other) => panic!("expected Encryption error, got: {:?}", other),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn test_encrypted_fts5_works() {
    let (_dir, path) = tmp_db();
    let mut config = Config::default();
    config.storage.encryption_enabled = true;

    let store = Store::open(&path, config, Some("fts5-test")).unwrap();

    store
        .save(SaveParams {
            key: "arch/pattern".into(),
            value: "Repository pattern for database access".into(),
            scope: Some("/myproject".into()),
            ..Default::default()
        })
        .unwrap();

    store
        .save(SaveParams {
            key: "arch/config".into(),
            value: "Configuration loading with TOML parser".into(),
            scope: Some("/myproject".into()),
            ..Default::default()
        })
        .unwrap();

    let results = store.search(search_query("repository")).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].value_preview.contains("Repository"));

    let results = store.search(search_query("TOML")).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].value_preview.contains("TOML"));
}

#[test]
fn test_is_encrypted() {
    let (_dir, path) = tmp_db();

    // Create unencrypted DB
    {
        let config = Config::default();
        let _store = Store::open(&path, config, None).unwrap();
    }
    assert!(!Store::is_encrypted(&path));

    // Create encrypted DB at different path
    let enc_path = format!("{}.enc", path);
    {
        let mut config = Config::default();
        config.storage.encryption_enabled = true;
        let _store = Store::open(&enc_path, config, Some("detect-test")).unwrap();
    }
    assert!(Store::is_encrypted(&enc_path));
}

#[test]
fn test_encrypt_existing_db() {
    let (_dir, path) = tmp_db();
    let config = Config::default();
    let passphrase = "migrate-test";

    // Create unencrypted DB with data
    {
        let store = Store::open(&path, config.clone(), None).unwrap();
        store
            .save(SaveParams {
                key: "test/migrate".into(),
                value: "data before encryption".into(),
                scope: Some("/".into()),
                ..Default::default()
            })
            .unwrap();
    }
    assert!(!Store::is_encrypted(&path));

    // Encrypt it
    Store::encrypt(&path, passphrase, config.clone()).unwrap();
    assert!(Store::is_encrypted(&path));

    // Verify data is intact
    {
        let mut enc_config = config.clone();
        enc_config.storage.encryption_enabled = true;
        let store = Store::open(&path, enc_config, Some(passphrase)).unwrap();
        let results = store.search(search_query("encryption")).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].value_preview.contains("data before encryption"));
    }
}

#[test]
fn test_decrypt_existing_db() {
    let (_dir, path) = tmp_db();
    let mut config = Config::default();
    config.storage.encryption_enabled = true;
    let passphrase = "decrypt-test";

    // Create encrypted DB with data
    {
        let store = Store::open(&path, config.clone(), Some(passphrase)).unwrap();
        store
            .save(SaveParams {
                key: "test/decrypt".into(),
                value: "data before decryption".into(),
                scope: Some("/".into()),
                ..Default::default()
            })
            .unwrap();
    }
    assert!(Store::is_encrypted(&path));

    // Decrypt it
    Store::decrypt(&path, passphrase, config).unwrap();
    assert!(!Store::is_encrypted(&path));

    // Verify data is intact without passphrase
    {
        let plain_config = Config::default();
        let store = Store::open(&path, plain_config, None).unwrap();
        let results = store.search(search_query("decryption")).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].value_preview.contains("data before decryption"));
    }
}

#[test]
fn test_passphrase_on_plaintext_db_returns_error_not_corruption() {
    // This is the exact scenario that corrupted the user's DB:
    // Config says encryption_enabled but DB is actually plaintext.
    // Store::open must fail cleanly (Encryption error), NOT corrupt the DB.
    let (_dir, path) = tmp_db();

    // Create a plaintext DB with real data
    {
        let config = Config::default();
        let store = Store::open(&path, config, None).unwrap();
        store
            .save(SaveParams {
                key: "important/data".into(),
                value: "this must survive".into(),
                scope: Some("/".into()),
                ..Default::default()
            })
            .unwrap();
    }

    // Try opening plaintext DB with a passphrase (mismatch scenario)
    let mut config = Config::default();
    config.storage.encryption_enabled = true;
    match Store::open(&path, config, Some("wrong-key")) {
        Err(Error::Encryption(_)) => {} // expected
        Err(other) => panic!("expected Encryption error, got: {:?}", other),
        Ok(_) => panic!("should have failed — passphrase on plaintext DB"),
    }

    // Verify DB is NOT corrupted — must still open without passphrase
    {
        let config = Config::default();
        let store = Store::open(&path, config, None).unwrap();
        let results = store.search(search_query("survive")).unwrap();
        assert_eq!(
            results.len(),
            1,
            "data must survive the failed open attempt"
        );
    }
}

#[test]
fn test_encrypt_already_encrypted_returns_error() {
    // Regression: calling encrypt() on an already-encrypted DB must return a
    // clear Encryption error, not SQLITE_NOTADB (error code 26).
    let (_dir, path) = tmp_db();
    let mut config = Config::default();
    config.storage.encryption_enabled = true;
    let passphrase = "already-enc";

    // Create an encrypted DB
    {
        let _store = Store::open(&path, config.clone(), Some(passphrase)).unwrap();
    }
    assert!(Store::is_encrypted(&path));

    // Calling encrypt() again must return Encryption error, not a raw DB error
    match Store::encrypt(&path, "new-passphrase", config) {
        Err(Error::Encryption(msg)) => assert!(msg.contains("already encrypted"), "msg: {}", msg),
        Err(other) => panic!("expected Encryption error, got: {:?}", other),
        Ok(_) => panic!("expected error — db is already encrypted"),
    }

    // Original db must still be intact and openable with original passphrase
    let mut check_config = Config::default();
    check_config.storage.encryption_enabled = true;
    Store::open(&path, check_config, Some(passphrase))
        .expect("original db must still be accessible");
}

#[test]
fn test_encrypt_cleans_up_stale_tmp_file() {
    // Regression: a stale .encrypting file from a previous failed attempt
    // must not block a fresh encryption run.
    let (_dir, path) = tmp_db();
    let config = Config::default();
    let passphrase = "stale-tmp-test";

    // Create plaintext DB with data
    {
        let store = Store::open(&path, config.clone(), None).unwrap();
        store
            .save(SaveParams {
                key: "test/stale".into(),
                value: "data must survive".into(),
                scope: Some("/".into()),
                ..Default::default()
            })
            .unwrap();
    }

    // Simulate a stale .encrypting file from a previous failed run
    let tmp_path = format!("{}.encrypting", path);
    std::fs::write(&tmp_path, b"garbage data from previous failed attempt").unwrap();
    assert!(std::path::Path::new(&tmp_path).exists());

    // Encrypt must succeed despite the stale file
    Store::encrypt(&path, passphrase, config.clone()).unwrap();
    assert!(Store::is_encrypted(&path));
    assert!(
        !std::path::Path::new(&tmp_path).exists(),
        "stale tmp must be cleaned up"
    );

    // Data must be intact
    let mut enc_config = config;
    enc_config.storage.encryption_enabled = true;
    let store = Store::open(&path, enc_config, Some(passphrase)).unwrap();
    let results = store.search(search_query("survive")).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn test_encrypt_preserves_all_data() {
    // Thorough data integrity check: multiple memories, sessions, metrics
    let (_dir, path) = tmp_db();
    let config = Config::default();
    let passphrase = "integrity-test";

    // Create DB with diverse data
    {
        let store = Store::open(&path, config.clone(), None).unwrap();
        for i in 0..10 {
            store
                .save(SaveParams {
                    key: format!("test/key-{}", i),
                    value: format!("value number {} with unique content xyz{}", i, i),
                    scope: Some("/project".into()),
                    ..Default::default()
                })
                .unwrap();
        }
    }

    // Encrypt
    Store::encrypt(&path, passphrase, config.clone()).unwrap();

    // Verify ALL data survived
    {
        let mut enc_config = config;
        enc_config.storage.encryption_enabled = true;
        let store = Store::open(&path, enc_config, Some(passphrase)).unwrap();

        let all = store.list(None, None, Some(100)).unwrap();
        assert_eq!(all.len(), 10, "all 10 memories must survive encryption");

        for i in 0..10 {
            let results = store.search(search_query(&format!("xyz{}", i))).unwrap();
            assert_eq!(
                results.len(),
                1,
                "memory {} must be searchable after encryption",
                i
            );
        }
    }
}
