#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("memory not found: {0}")]
    NotFound(i64),

    #[error("empty value: memory must contain something")]
    EmptyValue,

    #[error("empty key: key is required")]
    EmptyKey,

    #[error("key too long: {0} chars (max {1})")]
    KeyTooLong(usize, usize),

    #[error("too many tags: {0} (max {1})")]
    TooManyTags(usize, usize),

    #[error("tag too long: {0} chars (max {1})")]
    TagTooLong(usize, usize),

    #[error("invalid scope: {0}")]
    InvalidScope(String),

    #[error("invalid source type: {0}")]
    InvalidSourceType(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("schema version {found} is newer than supported version {supported}")]
    SchemaVersionTooNew { found: i64, supported: i64 },

    #[error("schema migration failed: {0}")]
    Migration(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("encryption error: {0}")]
    Encryption(String),

    #[error("duplicate memory: identical content already exists as id {0}")]
    Duplicate(i64),

    #[error("low information content (score: {0:.2})")]
    LowInformation(f64),
}

pub type Result<T> = std::result::Result<T, Error>;
