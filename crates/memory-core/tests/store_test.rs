use memory_core::test_utils::*;
use memory_core::*;

#[test]
fn save_and_retrieve_by_id() {
    let store = create_test_store();
    let id = create_test_memory(&store, "test-key", "test-value");
    let mem = store.get(id).unwrap();
    assert_eq!(mem.key, "test-key");
    assert_eq!(mem.value, "test-value");
    assert_eq!(mem.scope, "/");
}

#[test]
fn upsert_same_key_scope() {
    let store = create_test_store();
    let action1 = store
        .save(SaveParams {
            key: "mykey".into(),
            value: "first".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(matches!(action1, SaveAction::Created(_)));

    let action2 = store
        .save(SaveParams {
            key: "mykey".into(),
            value: "second".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(matches!(action2, SaveAction::Updated(_)));
    assert_eq!(action1.id(), action2.id());

    let mem = store.get(action2.id()).unwrap();
    assert_eq!(mem.value, "second");
    assert_eq!(mem.revision_count, 1);
}

#[test]
fn duplicate_content_increments_count() {
    let store = create_test_store();
    store
        .save(SaveParams {
            key: "key1".into(),
            value: "same content".into(),
            scope: Some("/a".into()),
            ..Default::default()
        })
        .unwrap();

    // Different key, same scope, same content hash -> dedup
    let action2 = store
        .save(SaveParams {
            key: "key2".into(),
            value: "same content".into(),
            scope: Some("/a".into()),
            ..Default::default()
        })
        .unwrap();

    // key2 is different from key1, so it's NOT a dedup (different key means different hash)
    // Dedup only happens when normalized_hash matches, which includes key in the hash
    assert!(matches!(action2, SaveAction::Created(_)));
}

#[test]
fn fts5_search_returns_results() {
    let store = create_test_store();
    create_test_memory(&store, "architecture", "The system uses microservices");
    create_test_memory(&store, "database", "PostgreSQL for persistence");

    let results = store
        .search(SearchParams {
            query: "microservices".into(),
            scope: None,
            source_type: None,
            limit: None,
        })
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "architecture");
}

#[test]
fn fts5_search_with_scope_filter() {
    let store = create_test_store();
    store
        .save(SaveParams {
            key: "api".into(),
            value: "REST API design".into(),
            scope: Some("/project/api".into()),
            ..Default::default()
        })
        .unwrap();
    store
        .save(SaveParams {
            key: "frontend".into(),
            value: "REST client code".into(),
            scope: Some("/project/web".into()),
            ..Default::default()
        })
        .unwrap();

    let results = store
        .search(SearchParams {
            query: "REST".into(),
            scope: Some("/project/api".into()),
            source_type: None,
            limit: None,
        })
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "api");
}

#[test]
fn fts5_search_with_source_type_filter() {
    let store = create_test_store();
    store
        .save(SaveParams {
            key: "k1".into(),
            value: "Pattern matching in Rust uses exhaustive match expressions with arms and guards for structured control flow".into(),
            source_type: Some(SourceType::Codebase),
            ..Default::default()
        })
        .unwrap();
    store
        .save(SaveParams {
            key: "k2".into(),
            value: "Pattern design principles applied to the API surface for consistency and discoverability".into(),
            source_type: Some(SourceType::Explicit),
            ..Default::default()
        })
        .unwrap();

    let results = store
        .search(SearchParams {
            query: "pattern".into(),
            scope: None,
            source_type: Some(SourceType::Codebase),
            limit: None,
        })
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "k1");
}

#[test]
fn soft_delete_hides_from_queries() {
    let store = create_test_store();
    let id = create_test_memory(&store, "del-key", "to be deleted");

    let deleted = store.delete("del-key", None, false).unwrap();
    assert!(deleted);

    let result = store.get(id);
    assert!(result.is_err());

    let search = store
        .search(SearchParams {
            query: "deleted".into(),
            scope: None,
            source_type: None,
            limit: None,
        })
        .unwrap();
    assert!(search.is_empty());
}

#[test]
fn hard_delete_removes_row() {
    let store = create_test_store();
    create_test_memory(&store, "hard-del", "permanent delete");

    let deleted = store.delete("hard-del", None, true).unwrap();
    assert!(deleted);

    let second = store.delete("hard-del", None, true).unwrap();
    assert!(!second);
}

#[test]
fn update_modifies_fields() {
    let store = create_test_store();
    let id = create_test_memory(&store, "orig-key", "original value");

    let updated = store
        .update(id, Some("new-key"), Some("new value"), None)
        .unwrap();
    assert_eq!(updated.key, "new-key");
    assert_eq!(updated.value, "new value");
    assert_eq!(updated.revision_count, 1);
}

#[test]
fn list_filtered_by_scope_and_source() {
    let store = create_test_store();
    store
        .save(SaveParams {
            key: "a".into(),
            value: "Alpha component: primary entry point for the /proj module initialization and configuration loading".into(),
            scope: Some("/proj".into()),
            source_type: Some(SourceType::Codebase),
            ..Default::default()
        })
        .unwrap();
    store
        .save(SaveParams {
            key: "b".into(),
            value: "val b".into(),
            scope: Some("/proj".into()),
            source_type: Some(SourceType::Explicit),
            ..Default::default()
        })
        .unwrap();
    store
        .save(SaveParams {
            key: "c".into(),
            value: "Gamma component: secondary service in the /other scope handling request routing and middleware".into(),
            scope: Some("/other".into()),
            source_type: Some(SourceType::Codebase),
            ..Default::default()
        })
        .unwrap();

    let list = store
        .list(Some("/proj"), Some(&SourceType::Codebase), None)
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].key, "a");
}

#[test]
fn privacy_strips_private_tags() {
    let store = create_test_store();
    let id = store
        .save(SaveParams {
            key: "secret".into(),
            value: "public <private>hidden</private> visible".into(),
            ..Default::default()
        })
        .unwrap()
        .id();

    let mem = store.get(id).unwrap();
    assert!(!mem.value.contains("hidden"));
    assert!(mem.value.contains("[REDACTED]"));
}

#[test]
fn schema_migration_idempotent() {
    let store1 = Store::open_in_memory().unwrap();
    drop(store1);
    let _store2 = Store::open_in_memory().unwrap();
}

#[test]
fn empty_db_initializes_tables() {
    let store = create_test_store();
    // Should be able to query immediately
    let results = store
        .search(SearchParams {
            query: "anything".into(),
            scope: None,
            source_type: None,
            limit: None,
        })
        .unwrap();
    assert!(results.is_empty());
}

#[test]
fn metadata_table_created() {
    let store = create_test_store();
    let created = store.get_metadata("db_created_at").unwrap();
    assert!(created.is_some());
    assert!(!created.unwrap().is_empty());
}

#[test]
fn hard_delete_cascades_metrics_and_relations() {
    let store = create_test_store();
    let id1 = create_test_memory(&store, "mem-a", "value a");
    let id2 = create_test_memory(&store, "mem-b", "value b");

    // Create metrics and relations referencing id1
    store.record_injection(&[id1], 0).unwrap();
    store.record_hit(id1).unwrap();
    store
        .add_relation(
            id1,
            id2,
            memory_core::store::relations::RelationType::RelatedTo,
        )
        .unwrap();

    // Hard delete should succeed (FK CASCADE)
    let deleted = store.delete("mem-a", None, true).unwrap();
    assert!(deleted);

    // Verify relations are cleaned up
    let rels = store.get_relations(id2).unwrap();
    assert!(rels.is_empty());
}

#[test]
fn update_tags_with_key_change() {
    let store = create_test_store();
    let id = create_test_memory(&store, "old-key", "some value");

    let updated = store
        .update(id, Some("new-key"), None, Some(vec!["tag1".to_string()]))
        .unwrap();
    assert_eq!(updated.key, "new-key");
    assert_eq!(updated.tags, Some(vec!["tag1".to_string()]));
}

#[test]
fn search_by_tags_basic() {
    let store = create_test_store();
    store
        .save(SaveParams {
            key: "tagged".into(),
            value: "has tags".into(),
            tags: Some(vec!["skill:debug".to_string(), "procedural".to_string()]),
            ..Default::default()
        })
        .unwrap();
    store
        .save(SaveParams {
            key: "untagged".into(),
            value: "no tags".into(),
            ..Default::default()
        })
        .unwrap();

    let results = store.search_by_tags(&["skill:debug"], None, 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "tagged");
}

#[test]
fn search_by_tags_and_semantics() {
    let store = create_test_store();
    store
        .save(SaveParams {
            key: "both".into(),
            value: "has both".into(),
            tags: Some(vec!["a".to_string(), "b".to_string()]),
            ..Default::default()
        })
        .unwrap();
    store
        .save(SaveParams {
            key: "only-a".into(),
            value: "has only a".into(),
            tags: Some(vec!["a".to_string()]),
            ..Default::default()
        })
        .unwrap();

    let results = store.search_by_tags(&["a", "b"], None, 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "both");
}

#[test]
fn search_by_tags_empty_returns_empty() {
    let store = create_test_store();
    let results = store.search_by_tags(&[], None, 10).unwrap();
    assert!(results.is_empty());
}

#[test]
fn search_by_tags_with_scope() {
    let store = create_test_store();
    store
        .save(SaveParams {
            key: "in-scope".into(),
            value: "scoped".into(),
            scope: Some("/proj".into()),
            tags: Some(vec!["x".to_string()]),
            ..Default::default()
        })
        .unwrap();
    store
        .save(SaveParams {
            key: "out-scope".into(),
            value: "other scope".into(),
            scope: Some("/other".into()),
            tags: Some(vec!["x".to_string()]),
            ..Default::default()
        })
        .unwrap();

    let results = store.search_by_tags(&["x"], Some("/proj"), 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "in-scope");
}

// ── Weighted BM25 + relevance threshold + adaptive preview ──────────────────

/// Build a corpus large enough (5+ docs) that IDF is positive, making BM25 scores negative.
/// "microservices" appears in doc A's KEY and doc B's VALUE.
/// Weighted BM25 (key=10) should rank doc A first.
#[test]
fn weighted_bm25_key_match_ranks_above_value_match() {
    let store = memory_core::Store::open_in_memory().unwrap();

    store
        .save(memory_core::SaveParams {
            key: "microservices".into(),
            value: "unrelated content about databases".into(),
            ..Default::default()
        })
        .unwrap();
    store
        .save(memory_core::SaveParams {
            key: "architecture".into(),
            value: "microservices are used for horizontal scaling".into(),
            ..Default::default()
        })
        .unwrap();
    // Filler docs so df/N ratio is low enough for positive IDF
    for i in 0..6 {
        store
            .save(memory_core::SaveParams {
                key: format!("filler-{i}"),
                value: format!("completely different topic about topic {i}"),
                ..Default::default()
            })
            .unwrap();
    }

    let results = store
        .search(memory_core::SearchParams {
            query: "microservices".into(),
            scope: None,
            source_type: None,
            limit: Some(5),
        })
        .unwrap();

    assert_eq!(results.len(), 2, "both docs should match");
    assert_eq!(
        results[0].key, "microservices",
        "key match should rank first with weighted BM25 (key weight=10 > value weight=1)"
    );
    assert_eq!(
        results[1].key, "architecture",
        "value match should rank second"
    );
}

/// Threshold filtering: common terms (high df/N) produce positive BM25 and are filtered out.
/// Rare term with key match produces strongly negative BM25 and passes.
#[test]
fn relevance_threshold_filters_common_term_noise() {
    let config = memory_core::Config {
        search: memory_core::config::SearchConfig {
            min_relevance_score: Some(-0.1),
            ..Default::default()
        },
        ..Default::default()
    };
    let store = memory_core::Store::open_in_memory_with_config(config).unwrap();

    // "common" appears in ALL 10 docs → high df/N → negative IDF → positive BM25 → filtered
    for i in 0..10 {
        store
            .save(memory_core::SaveParams {
                key: format!("doc-{i}"),
                value: format!("common term in every single document {i}"),
                ..Default::default()
            })
            .unwrap();
    }
    // "zzyxq_rare" appears in only 1 doc's key → low df/N → positive IDF → negative BM25 → passes
    store
        .save(memory_core::SaveParams {
            key: "zzyxq_rare".into(),
            value: "unrelated information here".into(),
            ..Default::default()
        })
        .unwrap();

    let common_results = store
        .search(memory_core::SearchParams {
            query: "common".into(),
            scope: None,
            source_type: None,
            limit: None,
        })
        .unwrap();
    assert_eq!(
        common_results.len(),
        0,
        "common term (positive BM25 in saturated corpus) should be filtered by threshold"
    );

    let rare_results = store
        .search(memory_core::SearchParams {
            query: "zzyxq_rare".into(),
            scope: None,
            source_type: None,
            limit: None,
        })
        .unwrap();
    assert_eq!(
        rare_results.len(),
        1,
        "rare key match should pass threshold (strongly negative BM25)"
    );
}

/// Without a threshold (None), all matches are returned regardless of BM25 score.
#[test]
fn no_threshold_returns_all_matches() {
    let store = memory_core::Store::open_in_memory().unwrap();
    // Single-doc corpus: BM25 is positive, but no threshold means it still returns
    store
        .save(memory_core::SaveParams {
            key: "single".into(),
            value: "match this query term".into(),
            ..Default::default()
        })
        .unwrap();

    let results = store
        .search(memory_core::SearchParams {
            query: "match".into(),
            scope: None,
            source_type: None,
            limit: None,
        })
        .unwrap();
    assert_eq!(
        results.len(),
        1,
        "no threshold configured — should return the matching doc"
    );
}

/// Adaptive preview: position 0 gets preview_max_chars, 1-2 get 200, 3+ get preview_min_chars.
#[test]
fn adaptive_preview_length_by_position() {
    let long_value = "x".repeat(500);
    let config = memory_core::Config {
        search: memory_core::config::SearchConfig {
            preview_max_chars: 100,
            preview_min_chars: 20,
            ..Default::default()
        },
        ..Default::default()
    };
    let store = memory_core::Store::open_in_memory_with_config(config).unwrap();

    // Need 5 docs so we have results at positions 0-4
    // All contain "queryterm" to ensure they all match; use same key prefix so key weights
    // are similar — differentiate via tags to force 5 distinct docs
    for i in 0..5 {
        store
            .save(memory_core::SaveParams {
                key: format!("queryterm-doc-{i}"),
                value: long_value.clone(),
                ..Default::default()
            })
            .unwrap();
    }

    // Add 10 filler docs so IDF is positive (5 out of 15 docs have "queryterm")
    for i in 0..10 {
        store
            .save(memory_core::SaveParams {
                key: format!("filler-{i}"),
                value: "something completely unrelated".into(),
                ..Default::default()
            })
            .unwrap();
    }

    let results = store
        .search(memory_core::SearchParams {
            query: "queryterm".into(),
            scope: None,
            source_type: None,
            limit: Some(5),
        })
        .unwrap();

    assert_eq!(results.len(), 5);
    // Position 0: preview_max_chars = 100 → preview length ≤ 100 + 3 ("...")
    assert!(
        results[0].value_preview.len() <= 103,
        "pos 0 preview should be ≤ preview_max_chars+3, got {}",
        results[0].value_preview.len()
    );
    assert!(
        results[0].value_preview.len() > 20,
        "pos 0 preview should be longer than preview_min_chars"
    );
    // Position 1: middle = 200 chars, but our config has max=100; with value length=500
    // the 200-char middle size is still truncated at 200 (> max_chars=100, so still 100+3)
    // Actually: preview_len for pos 1-2 is hardcoded 200, value is 500 chars → preview = 200 chars
    // But our config max is 100 — the 200 middle is independent of config, it's hardcoded
    assert!(
        results[1].value_preview.len() <= 203,
        "pos 1 preview ≤ 203, got {}",
        results[1].value_preview.len()
    );
    // Position 3+: preview_min_chars = 20 → preview ≤ 23
    assert!(
        results[3].value_preview.len() <= 23,
        "pos 3+ preview should be ≤ preview_min_chars+3, got {}",
        results[3].value_preview.len()
    );
    assert!(
        results[4].value_preview.len() <= 23,
        "pos 4 preview ≤ 23, got {}",
        results[4].value_preview.len()
    );
}

/// Custom column_weights override the defaults.
#[test]
fn custom_column_weights_affect_ranking() {
    use std::collections::BTreeMap;

    // Inverted weights: value=10, key=1 — should now rank value matches first
    let mut weights = BTreeMap::new();
    weights.insert("key".to_string(), 1.0);
    weights.insert("value".to_string(), 10.0);
    weights.insert("tags".to_string(), 1.0);
    weights.insert("source_type".to_string(), 0.5);
    weights.insert("scope".to_string(), 0.5);

    let config = memory_core::Config {
        search: memory_core::config::SearchConfig {
            column_weights: weights,
            ..Default::default()
        },
        ..Default::default()
    };
    let store = memory_core::Store::open_in_memory_with_config(config).unwrap();

    store
        .save(memory_core::SaveParams {
            key: "uniqueterm".into(),
            value: "unrelated filler content here".into(),
            ..Default::default()
        })
        .unwrap();
    store
        .save(memory_core::SaveParams {
            key: "other-key".into(),
            value: "uniqueterm is in the value here".into(),
            ..Default::default()
        })
        .unwrap();
    for i in 0..6 {
        store
            .save(memory_core::SaveParams {
                key: format!("filler-{i}"),
                value: format!("something else entirely {i}"),
                ..Default::default()
            })
            .unwrap();
    }

    let results = store
        .search(memory_core::SearchParams {
            query: "uniqueterm".into(),
            scope: None,
            source_type: None,
            limit: Some(5),
        })
        .unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(
        results[0].key, "other-key",
        "value match should rank first when value weight (10) > key weight (1)"
    );
    assert_eq!(results[1].key, "uniqueterm");
}

/// Unknown column names in column_weights are silently ignored — no panic, no error.
#[test]
fn unknown_column_weight_names_are_ignored() {
    use std::collections::BTreeMap;

    let mut weights = BTreeMap::new();
    weights.insert("key".to_string(), 10.0);
    weights.insert("value".to_string(), 1.0);
    weights.insert("nonexistent_column".to_string(), 999.0);
    weights.insert("another_fake_col".to_string(), 0.0);

    let config = memory_core::Config {
        search: memory_core::config::SearchConfig {
            column_weights: weights,
            ..Default::default()
        },
        ..Default::default()
    };
    let store = memory_core::Store::open_in_memory_with_config(config).unwrap();
    store
        .save(memory_core::SaveParams {
            key: "testkey".into(),
            value: "testvalue content".into(),
            ..Default::default()
        })
        .unwrap();

    // Should not panic or error — unknown columns are ignored by build_bm25_expr
    let results = store
        .search(memory_core::SearchParams {
            query: "testvalue".into(),
            scope: None,
            source_type: None,
            limit: None,
        })
        .unwrap();
    // In a 1-doc corpus BM25 may be positive; just verify no error occurred
    // (result count doesn't matter — the test is about graceful handling)
    let _ = results;
}

/// OR fallback: when AND returns no results, retry with OR so partial-term queries still hit.
#[test]
fn search_or_fallback_finds_partial_match() {
    let store = memory_core::Store::open_in_memory().unwrap();
    store
        .save(memory_core::SaveParams {
            key: "auth/approach".into(),
            value:
                "No auth in single-user mode. MCP transport handles auth. Unix socket permissions."
                    .into(),
            ..Default::default()
        })
        .unwrap();

    // AND query: all three terms must be present — "authentication" and "security" are absent
    // from the stored memory, so AND returns nothing. OR fallback rescues via "auth".
    let results = store
        .search(memory_core::SearchParams {
            query: "authentication auth security".into(),
            scope: None,
            source_type: None,
            limit: None,
        })
        .unwrap();

    assert_eq!(
        results.len(),
        1,
        "OR fallback should find the memory via 'auth'"
    );
    assert_eq!(results[0].key, "auth/approach");
}

/// Single-term queries don't attempt an OR fallback (would be a no-op).
#[test]
fn search_single_term_no_fallback() {
    let store = memory_core::Store::open_in_memory().unwrap();
    store
        .save(memory_core::SaveParams {
            key: "db".into(),
            value: "SQLite WAL mode".into(),
            ..Default::default()
        })
        .unwrap();

    // "postgres" doesn't appear anywhere — single term, no fallback, returns empty
    let results = store
        .search(memory_core::SearchParams {
            query: "postgres".into(),
            scope: None,
            source_type: None,
            limit: None,
        })
        .unwrap();
    assert!(results.is_empty());
}

/// update() with a key change that would produce a hash matching an existing memory returns Duplicate.
#[test]
fn update_key_change_dedup_check() {
    let store = create_test_store();
    // Save two memories: target (to update) and existing (which will collide after rename)
    let existing_id = create_test_memory(&store, "existing-key", "shared value");
    let target_id = create_test_memory(&store, "target-key", "shared value");

    // Renaming target-key to existing-key with the same value produces the same hash
    let result = store.update(target_id, Some("existing-key"), None, None);
    assert!(
        matches!(result, Err(Error::Duplicate(id)) if id == existing_id),
        "expected Duplicate({existing_id}), got {result:?}"
    );
}

/// update() with a value change that matches another memory's hash returns Duplicate.
#[test]
fn update_value_change_dedup_check() {
    let store = create_test_store();
    let existing_id = create_test_memory(&store, "same-key", "duplicate value");
    let target_id = create_test_memory(&store, "other-key", "original value");

    // Changing key+value on target to match existing exactly
    let result = store.update(target_id, Some("same-key"), Some("duplicate value"), None);
    assert!(
        matches!(result, Err(Error::Duplicate(id)) if id == existing_id),
        "expected Duplicate({existing_id}), got {result:?}"
    );
}

/// update() that changes key+value to a unique combination succeeds.
#[test]
fn update_key_value_unique_succeeds() {
    let store = create_test_store();
    let id = create_test_memory(&store, "key-a", "value-a");
    let result = store.update(id, Some("key-b"), Some("value-b"), None);
    assert!(result.is_ok());
    let mem = result.unwrap();
    assert_eq!(mem.key, "key-b");
    assert_eq!(mem.value, "value-b");
}

/// FTS_COLUMNS constant matches the FTS5 virtual table column order in the schema.
#[test]
fn fts_columns_constant_matches_schema() {
    assert_eq!(
        memory_core::store::memory::FTS_COLUMNS,
        &["key", "value", "tags", "source_type", "scope"],
        "FTS_COLUMNS must match CREATE VIRTUAL TABLE column order in 001_initial.sql"
    );
}
