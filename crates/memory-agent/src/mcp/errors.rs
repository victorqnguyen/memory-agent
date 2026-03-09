use rmcp::model::ErrorCode;
use rmcp::ErrorData as McpError;
use serde_json::Value;

fn mcp_err(code: i32, msg: impl Into<String>) -> McpError {
    McpError::new(ErrorCode(code), msg.into(), None::<Value>)
}

pub fn to_mcp_error(err: anyhow::Error) -> McpError {
    let msg = err.to_string();

    if let Some(core_err) = err.downcast_ref::<memory_core::Error>() {
        return match core_err {
            memory_core::Error::NotFound(_) => mcp_err(-32001, "memory not found"),
            memory_core::Error::EmptyKey => mcp_err(-32602, "invalid params: key is required"),
            memory_core::Error::EmptyValue => mcp_err(-32602, "invalid params: value is required"),
            memory_core::Error::KeyTooLong(_, max) => {
                mcp_err(-32602, format!("invalid params: key exceeds {max} chars"))
            }
            memory_core::Error::InvalidScope(reason) => {
                mcp_err(-32602, format!("invalid params: scope {reason}"))
            }
            memory_core::Error::TooManyTags(_, max) => {
                mcp_err(-32602, format!("invalid params: max {max} tags"))
            }
            memory_core::Error::TagTooLong(_, max) => {
                mcp_err(-32602, format!("invalid params: tag exceeds {max} chars"))
            }
            memory_core::Error::InvalidSourceType(s) => mcp_err(
                -32602,
                format!("invalid params: unknown source_type \"{s}\""),
            ),
            memory_core::Error::SchemaVersionTooNew { .. } => {
                mcp_err(-32000, "server error: database requires newer binary")
            }
            memory_core::Error::InvalidInput(_) => mcp_err(-32602, "invalid params: invalid input"),
            memory_core::Error::LowInformation(score) => mcp_err(
                -32602,
                format!(
                    "content rejected: low information density (score: {score:.2}). \
                     Use source_type 'explicit' to bypass."
                ),
            ),
            _ => {
                tracing::debug!("Database error details: {msg}");
                tracing::error!("Database error occurred");
                mcp_err(-32000, "server error: database error")
            }
        };
    }

    tracing::debug!("Internal error details: {msg}");
    tracing::error!("Internal error occurred");
    mcp_err(-32000, "server error: internal error")
}
