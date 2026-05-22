use std::sync::Arc;

use anyhow::Result;
use rmcp::transport::{
    StreamableHttpServerConfig, StreamableHttpService,
    streamable_http_server::session::local::LocalSessionManager,
};
use tracing::{info, warn};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;

use crate::config::args::Args;
use crate::config::tool::ToolsConfig;
use crate::mcp::tools::DynamicTools;

pub mod config;
pub mod db;
pub mod mcp;

#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_timer(LocalTimer)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_line_number(true)
        .with_level(true)
        .init();
    info!("start db mcp tools");

    let config = Arc::new(config::load_config());
    let result = mcp_servers(config.clone()).await;
    if let Err(error) = result {
        warn!("mcp服务失败: err:{}", error);
    }
    Ok(())
}

async fn mcp_servers(config: Arc<ToolsConfig>) -> anyhow::Result<()> {
    let ct = tokio_util::sync::CancellationToken::new();

    let service = StreamableHttpService::new(
        move || Ok(DynamicTools::new(config.clone())),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig::default().with_cancellation_token(ct.child_token()),
    );

    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3014").await?;
    let _ = axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            tokio::signal::ctrl_c().await.unwrap();
            ct.cancel();
        })
        .await;

    Ok(())
}

struct LocalTimer;

const fn east_utf8() -> Option<chrono::FixedOffset> {
    chrono::FixedOffset::east_opt(8 * 3600)
}

impl FormatTime for LocalTimer {
    fn format_time(&self, w: &mut Writer<'_>) -> std::fmt::Result {
        let now = chrono::Utc::now().with_timezone(&east_utf8().unwrap());
        write!(w, "{}", now.format("%FT%T%.3f"))
    }
}
