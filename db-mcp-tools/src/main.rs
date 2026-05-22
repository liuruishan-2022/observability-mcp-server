use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use rmcp::transport::{
    StreamableHttpServerConfig, StreamableHttpService,
    streamable_http_server::session::local::LocalSessionManager,
};
use tracing::{info, warn};
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;

use crate::config::args::Args;
use crate::db::database::DatabaseExecutor;
use crate::mcp::tools::{DynamicTools, HotToolsState};

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

    let args = Args::parse_args();
    info!("args:{}", args.to_string());
    let config_path = args.config().to_string();
    let config = config::load_config(&config_path)?;
    let database_executor = Arc::new(DatabaseExecutor::new().await);
    let state = Arc::new(HotToolsState::new(config, database_executor.clone()));
    tokio::spawn(watch_tools_config(config_path, state.clone()));
    let result = mcp_servers(state.clone()).await;
    if let Err(error) = result {
        warn!("mcp服务失败: err:{}", error);
    }
    Ok(())
}

async fn mcp_servers(state: Arc<HotToolsState>) -> anyhow::Result<()> {
    let ct = tokio_util::sync::CancellationToken::new();

    let service = StreamableHttpService::new(
        move || Ok(DynamicTools::new(state.clone())),
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

async fn watch_tools_config(config_path: String, state: Arc<HotToolsState>) {
    let mut last_content = tokio::fs::read_to_string(&config_path)
        .await
        .unwrap_or_default();

    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;

        let content = match tokio::fs::read_to_string(&config_path).await {
            Ok(content) => content,
            Err(error) => {
                warn!("read tools config failed: path:{config_path}, err:{error}");
                continue;
            }
        };

        if content == last_content {
            continue;
        }

        match serde_yaml::from_str(&content) {
            Ok(config) => {
                state.replace_config(config).await;
                state.notify_tools_changed().await;
                last_content = content;
                info!("tools config reloaded: path:{config_path}");
            }
            Err(error) => {
                warn!(
                    "parse tools config failed, keep previous config: path:{config_path}, err:{error}"
                );
            }
        }
    }
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
