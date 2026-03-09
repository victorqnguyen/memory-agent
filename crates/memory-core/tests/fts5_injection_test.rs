use memory_core::test_utils::*;
use memory_core::types::*;

#[test]
fn sql_injection_via_search() {
    let store = create_test_store();
    create_test_memory(&store, "normal", "some data");

    let results = store
        .search(SearchParams {
            query: "'; DROP TABLE memories; --".into(),
            scope: None,
            source_type: None,
            limit: None,
        })
        .unwrap();

    assert!(results.is_empty());

    // Verify table still exists
    let after = store
        .search(SearchParams {
            query: "data".into(),
            scope: None,
            source_type: None,
            limit: None,
        })
        .unwrap();
    assert_eq!(after.len(), 1);
}

#[test]
fn fts5_operator_injection() {
    let store = create_test_store();
    create_test_memory(&store, "normal", "some data");

    let results = store
        .search(SearchParams {
            query: "key:* OR 1=1".into(),
            scope: None,
            source_type: None,
            limit: None,
        })
        .unwrap();

    assert!(results.is_empty());
}

#[test]
fn fts5_near_injection() {
    let store = create_test_store();
    create_test_memory(&store, "normal", "password admin");

    // NEAR operator is stripped, but "password" and "admin" become safe quoted terms
    // The search should succeed without error (no FTS5 syntax crash)
    let results = store
        .search(SearchParams {
            query: "NEAR(password admin)".into(),
            scope: None,
            source_type: None,
            limit: None,
        })
        .unwrap();

    // It finds the memory via the remaining quoted terms — that's safe behavior
    // The key assertion is that it doesn't crash with an FTS5 syntax error
    assert!(!results.is_empty());
}

#[test]
fn fts5_near_pure_operator_returns_empty() {
    let store = create_test_store();
    create_test_memory(&store, "normal", "some data");

    // Pure NEAR with no terms after stripping
    let results = store
        .search(SearchParams {
            query: "NEAR()".into(),
            scope: None,
            source_type: None,
            limit: None,
        })
        .unwrap();

    assert!(results.is_empty());
}
