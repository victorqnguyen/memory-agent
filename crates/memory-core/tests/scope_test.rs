use memory_core::*;
use memory_core::test_utils::*;

fn save_at_scope(store: &Store, key: &str, value: &str, scope: &str) -> i64 {
    store
        .save(SaveParams {
            key: key.to_string(),
            value: value.to_string(),
            scope: Some(scope.to_string()),
            ..Default::default()
        })
        .unwrap()
        .id()
}

#[test]
fn search_inherits_from_parent_scope() {
    let store = create_test_store();
    save_at_scope(&store, "global-rule", "always use tabs", "/");
    save_at_scope(&store, "project-rule", "use bun test", "/project/api");

    let results = store
        .search(SearchParams {
            query: "rule".to_string(),
            scope: Some("/project/api".to_string()),
            source_type: None,
            limit: None,
        })
        .unwrap();

    assert_eq!(results.len(), 2);
    // Most specific scope first
    assert_eq!(results[0].scope, "/project/api");
    assert_eq!(results[1].scope, "/");
}

#[test]
fn search_does_not_see_sibling_scope() {
    let store = create_test_store();
    save_at_scope(&store, "web-rule", "use react", "/project/web");
    save_at_scope(&store, "api-rule", "use axum", "/project/api");

    let results = store
        .search(SearchParams {
            query: "rule".to_string(),
            scope: Some("/project/api".to_string()),
            source_type: None,
            limit: None,
        })
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "api-rule");
}

#[test]
fn list_inherits_from_ancestors() {
    let store = create_test_store();
    save_at_scope(&store, "root-mem", "root value", "/");
    save_at_scope(&store, "org-mem", "org value", "/org");
    save_at_scope(&store, "project-mem", "project value", "/org/project");

    let results = store.list(Some("/org/project"), None, None).unwrap();
    assert_eq!(results.len(), 3);
    // Most specific first
    assert_eq!(results[0].scope, "/org/project");
    assert_eq!(results[1].scope, "/org");
    assert_eq!(results[2].scope, "/");
}

#[test]
fn list_does_not_see_child_scope() {
    let store = create_test_store();
    save_at_scope(&store, "parent", "parent val", "/org");
    save_at_scope(&store, "child", "child val", "/org/project");

    let results = store.list(Some("/org"), None, None).unwrap();
    // /org can see root and /org, but NOT /org/project
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "parent");
}

#[test]
fn context_nearest_match_dedup() {
    let store = create_test_store();
    // Same key at two scopes
    save_at_scope(&store, "test-cmd", "npm test", "/");
    save_at_scope(&store, "test-cmd", "bun test", "/project/api");

    let results = store.context(Some("/project/api"), None).unwrap();

    // Should only return the most specific match
    let test_cmds: Vec<&Memory> = results.iter().filter(|m| m.key == "test-cmd").collect();
    assert_eq!(test_cmds.len(), 1);
    assert_eq!(test_cmds[0].value, "bun test");
    assert_eq!(test_cmds[0].scope, "/project/api");
}

#[test]
fn context_without_scope_returns_all() {
    let store = create_test_store();
    save_at_scope(&store, "mem1", "val1", "/");
    save_at_scope(&store, "mem2", "val2", "/project");

    let results = store.context(None, None).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn root_scope_sees_only_root() {
    let store = create_test_store();
    save_at_scope(&store, "root-only", "at root", "/");
    save_at_scope(&store, "deep", "deep value", "/a/b/c");

    let results = store.list(Some("/"), None, None).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "root-only");
}
