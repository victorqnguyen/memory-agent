use memory_core::config::*;
use memory_core::Store;

#[test]
fn config_default_has_sane_values() {
    let config = Config::default();
    assert_eq!(config.validation.max_key_length, 256);
    assert_eq!(config.validation.max_value_length, 2000);
    assert_eq!(config.validation.max_tags, 20);
    assert_eq!(config.validation.max_tag_length, 64);
    assert_eq!(config.search.default_limit, 10);
    assert_eq!(config.search.max_limit, 50);
    assert_eq!(config.storage.retention_days, 90);
    assert_eq!(config.storage.dedup_window_secs, 900);
    assert!(!config.privacy.secret_patterns.is_empty());
}

#[test]
fn store_open_with_default_config() {
    let _store = Store::open_in_memory().unwrap();
}

#[test]
fn store_open_with_custom_config() {
    let mut config = Config::default();
    config.validation.max_key_length = 128;
    let store = Store::open_in_memory_with_config(config).unwrap();
    assert_eq!(store.config().validation.max_key_length, 128);
}

#[test]
fn validation_limits_from_config() {
    let mut config = Config::default();
    config.validation.max_key_length = 10;
    let store = Store::open_in_memory_with_config(config).unwrap();

    let result = store.save(memory_core::SaveParams {
        key: "a".repeat(11),
        value: "val".into(),
        ..Default::default()
    });

    assert!(matches!(result, Err(memory_core::Error::KeyTooLong(11, 10))));
}

#[test]
fn privacy_patterns_from_config() {
    let mut config = Config::default();
    config.privacy.extra_patterns.push(r"CUSTOM_SECRET_\d+".into());
    let store = Store::open_in_memory_with_config(config).unwrap();

    let id = store
        .save(memory_core::SaveParams {
            key: "test".into(),
            value: "has CUSTOM_SECRET_42 inside".into(),
            ..Default::default()
        })
        .unwrap()
        .id();

    let mem = store.get(id).unwrap();
    assert!(mem.value.contains("[SECRET_REDACTED]"));
}

#[test]
fn search_defaults_from_config() {
    let mut config = Config::default();
    config.search.default_limit = 5;
    let store = Store::open_in_memory_with_config(config).unwrap();
    assert_eq!(store.config().search.default_limit, 5);
}
