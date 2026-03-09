use memory_core::{
    store::Store,
    types::{SaveAction, SaveParams, SearchParams, SourceType},
};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

#[derive(Deserialize)]
struct SaveParamsJson {
    key: String,
    value: String,
    scope: Option<String>,
    source_type: Option<String>,
    source_ref: Option<String>,
    source_commit: Option<String>,
    tags: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct SearchParamsJson {
    query: String,
    scope: Option<String>,
    source_type: Option<String>,
    limit: Option<i32>,
}

#[derive(Serialize)]
struct SaveActionJson {
    action: String,
    id: i64,
}

impl From<SaveAction> for SaveActionJson {
    fn from(a: SaveAction) -> Self {
        Self {
            action: a.action_str().to_string(),
            id: a.id(),
        }
    }
}

#[wasm_bindgen]
pub struct MemoryEngine {
    store: Store,
}

#[wasm_bindgen]
impl MemoryEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<MemoryEngine, JsError> {
        let store = Store::open_in_memory().map_err(|e| JsError::new(&e.to_string()))?;
        Ok(Self { store })
    }

    /// Save a memory. `params_json` is a JSON object with fields: key, value, scope,
    /// source_type, source_ref, source_commit, tags.
    /// Returns JSON: `{"action": "created"|"updated"|"deduplicated", "id": <i64>}`.
    pub fn save(&mut self, params_json: &str) -> Result<String, JsError> {
        let p: SaveParamsJson =
            serde_json::from_str(params_json).map_err(|e| JsError::new(&e.to_string()))?;

        let source_type = p
            .source_type
            .as_deref()
            .map(|s| s.parse::<SourceType>())
            .transpose()
            .map_err(|e| JsError::new(&e.to_string()))?;

        let params = SaveParams {
            key: p.key,
            value: p.value,
            scope: p.scope,
            source_type,
            source_ref: p.source_ref,
            source_commit: p.source_commit,
            tags: p.tags,
        };

        let action = self
            .store
            .save(params)
            .map_err(|e| JsError::new(&e.to_string()))?;

        let result = SaveActionJson::from(action);
        serde_json::to_string(&result).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Search memories. `params_json` is a JSON object with fields: query, scope,
    /// source_type, limit.
    /// Returns a JSON array of `SearchResult`.
    pub fn search(&self, params_json: &str) -> Result<String, JsError> {
        let p: SearchParamsJson =
            serde_json::from_str(params_json).map_err(|e| JsError::new(&e.to_string()))?;

        let source_type = p
            .source_type
            .as_deref()
            .map(|s| s.parse::<SourceType>())
            .transpose()
            .map_err(|e| JsError::new(&e.to_string()))?;

        let params = SearchParams {
            query: p.query,
            scope: p.scope,
            source_type,
            limit: p.limit,
        };

        let results = self
            .store
            .search(params)
            .map_err(|e| JsError::new(&e.to_string()))?;

        serde_json::to_string(&results).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Get a single memory by ID. Returns JSON `Memory` or an error if not found.
    pub fn get(&self, id: i64) -> Result<String, JsError> {
        let memory = self
            .store
            .get(id)
            .map_err(|e| JsError::new(&e.to_string()))?;

        serde_json::to_string(&memory).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Get context memories for a scope. Returns a JSON array of `Memory`.
    pub fn context(&self, scope: Option<String>, limit: Option<i32>) -> Result<String, JsError> {
        let memories = self
            .store
            .context(scope.as_deref(), limit)
            .map_err(|e| JsError::new(&e.to_string()))?;

        serde_json::to_string(&memories).map_err(|e| JsError::new(&e.to_string()))
    }

    /// List memories. Returns a JSON array of `Memory`.
    pub fn list(&self, scope: Option<String>, limit: Option<i32>) -> Result<String, JsError> {
        let memories = self
            .store
            .list(scope.as_deref(), None, limit)
            .map_err(|e| JsError::new(&e.to_string()))?;

        serde_json::to_string(&memories).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Delete a memory by key (and optional scope). Returns `true` if deleted.
    pub fn delete(&mut self, key: &str, scope: Option<String>) -> Result<bool, JsError> {
        self.store
            .delete(key, scope.as_deref(), false)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Export all memories as a JSON array.
    pub fn export_all(&self) -> Result<String, JsError> {
        let memories = self
            .store
            .list_all(None, None)
            .map_err(|e| JsError::new(&e.to_string()))?;

        serde_json::to_string(&memories).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Import memories from a JSON array. Returns the number of memories imported.
    pub fn import_all(&mut self, json: &str) -> Result<i32, JsError> {
        #[derive(Deserialize)]
        struct ImportMemory {
            key: String,
            value: String,
            scope: String,
            source_type: SourceType,
            source_ref: Option<String>,
            source_commit: Option<String>,
            tags: Option<Vec<String>>,
        }

        let items: Vec<ImportMemory> =
            serde_json::from_str(json).map_err(|e| JsError::new(&e.to_string()))?;

        let mut count = 0i32;
        for item in items {
            let params = SaveParams {
                key: item.key,
                value: item.value,
                scope: Some(item.scope),
                source_type: Some(item.source_type),
                source_ref: item.source_ref,
                source_commit: item.source_commit,
                tags: item.tags,
            };
            self.store
                .save(params)
                .map_err(|e| JsError::new(&e.to_string()))?;
            count += 1;
        }

        Ok(count)
    }
}

impl Default for MemoryEngine {
    fn default() -> Self {
        Self::new().expect("failed to open in-memory store")
    }
}
