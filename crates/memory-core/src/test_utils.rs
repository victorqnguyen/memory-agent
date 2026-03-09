use crate::store::Store;
use crate::types::SaveParams;

pub fn create_test_store() -> Store {
    Store::open_in_memory().unwrap()
}

pub fn create_test_memory(store: &Store, key: &str, value: &str) -> i64 {
    store
        .save(SaveParams {
            key: key.to_string(),
            value: value.to_string(),
            ..Default::default()
        })
        .unwrap()
        .id()
}
