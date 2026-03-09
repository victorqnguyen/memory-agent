use memory_core::test_utils::*;
use memory_core::types::*;

#[test]
fn aws_access_key_redacted() {
    let store = create_test_store();
    let id = store
        .save(SaveParams {
            key: "aws".into(),
            value: "my key AKIAIOSFODNN7EXAMPLE here".into(),
            ..Default::default()
        })
        .unwrap()
        .id();

    let mem = store.get(id).unwrap();
    assert!(mem.value.contains("[SECRET_REDACTED]"));
    assert!(!mem.value.contains("AKIAIOSFODNN7EXAMPLE"));
}

#[test]
fn private_key_redacted() {
    let store = create_test_store();
    let id = store
        .save(SaveParams {
            key: "pkey".into(),
            value: "-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIB...".into(),
            ..Default::default()
        })
        .unwrap()
        .id();

    let mem = store.get(id).unwrap();
    assert!(mem.value.contains("[SECRET_REDACTED]"));
    assert!(!mem.value.contains("BEGIN RSA PRIVATE KEY"));
}

#[test]
fn api_key_assignment_redacted() {
    let store = create_test_store();
    let id = store
        .save(SaveParams {
            key: "config".into(),
            value: "api_key=abc123xyz secret here".into(),
            ..Default::default()
        })
        .unwrap()
        .id();

    let mem = store.get(id).unwrap();
    assert!(mem.value.contains("[SECRET_REDACTED]"));
}
