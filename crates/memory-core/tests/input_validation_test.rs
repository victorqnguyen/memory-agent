use memory_core::test_utils::*;
use memory_core::types::*;
use memory_core::Error;

#[test]
fn path_traversal_rejected() {
    let store = create_test_store();
    let result = store.save(SaveParams {
        key: "test".into(),
        value: "test".into(),
        scope: Some("../../etc/passwd".into()),
        ..Default::default()
    });
    assert!(matches!(result, Err(Error::InvalidScope(_))));
}

#[test]
fn null_byte_in_scope_rejected() {
    let store = create_test_store();
    let result = store.save(SaveParams {
        key: "test".into(),
        value: "test".into(),
        scope: Some("foo\0bar".into()),
        ..Default::default()
    });
    assert!(matches!(result, Err(Error::InvalidScope(_))));
}

#[test]
fn key_too_long_rejected() {
    let store = create_test_store();
    let result = store.save(SaveParams {
        key: "a".repeat(300),
        value: "test".into(),
        ..Default::default()
    });
    assert!(matches!(result, Err(Error::KeyTooLong(300, 256))));
}

#[test]
fn empty_value_rejected() {
    let store = create_test_store();
    let result = store.save(SaveParams {
        key: "test".into(),
        value: "".into(),
        ..Default::default()
    });
    assert!(matches!(result, Err(Error::EmptyValue)));
}

#[test]
fn too_many_tags_rejected() {
    let store = create_test_store();
    let result = store.save(SaveParams {
        key: "test".into(),
        value: "test".into(),
        tags: Some(vec!["t".into(); 25]),
        ..Default::default()
    });
    assert!(matches!(result, Err(Error::TooManyTags(25, 20))));
}

#[test]
fn empty_key_rejected() {
    let store = create_test_store();
    let result = store.save(SaveParams {
        key: "".into(),
        value: "test".into(),
        ..Default::default()
    });
    assert!(matches!(result, Err(Error::EmptyKey)));
}

#[test]
fn whitespace_only_key_rejected() {
    let store = create_test_store();
    let result = store.save(SaveParams {
        key: "   ".into(),
        value: "test".into(),
        ..Default::default()
    });
    assert!(matches!(result, Err(Error::EmptyKey)));
}

#[test]
fn whitespace_only_value_rejected() {
    let store = create_test_store();
    let result = store.save(SaveParams {
        key: "test".into(),
        value: "   ".into(),
        ..Default::default()
    });
    assert!(matches!(result, Err(Error::EmptyValue)));
}
