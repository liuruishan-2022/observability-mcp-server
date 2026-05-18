use anyhow::Result;
use dotenv::dotenv;
use rmcp::transport::{
    StreamableHttpServerConfig, StreamableHttpService,
    streamable_http_server::session::local::LocalSessionManager,
};
use tracing::info;
use tracing_subscriber::fmt::{format::Writer, time::FormatTime};

use crate::mcp::tools::Tools;

pub mod docs;
pub mod mcp;
pub mod searcher;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt()
        .with_timer(LocalTimer)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_line_number(true)
        .init();
    info!("start observability mcp server...");

    let ct = tokio_util::sync::CancellationToken::new();

    let mut server_config = StreamableHttpServerConfig::default().disable_allowed_hosts();
    server_config.cancellation_token = ct.child_token();
    info!("disabled MCP HTTP Host header allowlist");

    let service = StreamableHttpService::new(
        || Ok(Tools::new()),
        LocalSessionManager::default().into(),
        server_config,
    );

    let router = axum::Router::new().nest_service("/mcp", service);

    let tcp_listener = tokio::net::TcpListener::bind("0.0.0.0:3013").await?;
    let _ = axum::serve(tcp_listener, router)
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
