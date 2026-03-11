pub mod errors;
pub mod tools;
pub mod types;

use rmcp::ServiceExt;

use crate::async_store::AsyncStore;
use crate::llm::LlmTier;
use tools::MemoryServer;

/// Run background maintenance tasks on startup. Errors are logged, never propagated.
async fn run_startup_maintenance(store: AsyncStore) {
    // 1. Apply confidence decay
    match store.apply_confidence_decay().await {
        Ok(n) if n > 0 => tracing::info!("startup maintenance: confidence decay applied to {} memory/memories", n),
        Ok(_) => {}
        Err(e) => tracing::warn!("startup maintenance: confidence decay failed: {e}"),
    }

    // 3. Purge soft-deleted past retention window
    let retention = store.retention_days().await;
    match store.purge_soft_deleted(retention).await {
        Ok(n) if n > 0 => tracing::info!("startup maintenance: purged {} soft-deleted memory/memories", n),
        Ok(_) => {}
        Err(e) => tracing::warn!("startup maintenance: purge failed: {e}"),
    }

    // 4. Vacuum if overdue
    match store.maintenance_status().await {
        Ok(status) if status.vacuum_overdue => {
            tracing::info!("startup maintenance: running VACUUM (overdue)");
            if let Err(e) = store.vacuum().await {
                tracing::warn!("startup maintenance: VACUUM failed: {e}");
            }
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("startup maintenance: status check failed: {e}"),
    }
}

pub async fn run_mcp_server(store: AsyncStore, llm: LlmTier, max_value_length: usize) -> anyhow::Result<()> {
    tracing::info!("Starting MCP server on stdio");

    let server = MemoryServer::new(store.clone(), llm, max_value_length);

    // Spawn maintenance in background — does not delay MCP handshake
    tokio::spawn(run_startup_maintenance(store));

    let transport = rmcp::transport::io::stdio();

    let service = server.serve(transport).await?;

    service.waiting().await?;

    Ok(())
}
