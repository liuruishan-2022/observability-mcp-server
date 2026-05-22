use std::sync::Arc;

use crate::config::tool::ToolsConfig;
use crate::db::database::DatabaseExecutor;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ListToolsResult, PaginatedRequestParams,
};
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo, ToolsCapability},
    service::{MaybeSendFuture, NotificationContext, Peer, RequestContext},
};
use tokio::sync::RwLock;
use tracing::warn;

///
/// 使用动态的tools做自定义的配置形式
///

pub struct HotToolsState {
    config: RwLock<Arc<ToolsConfig>>,
    database_executor: Arc<DatabaseExecutor>,
    peers: RwLock<Vec<Peer<RoleServer>>>,
}

impl HotToolsState {
    pub fn new(config: ToolsConfig, database_executor: Arc<DatabaseExecutor>) -> Self {
        Self {
            config: RwLock::new(Arc::new(config)),
            database_executor,
            peers: RwLock::new(Vec::new()),
        }
    }

    pub async fn replace_config(&self, config: ToolsConfig) {
        *self.config.write().await = Arc::new(config);
    }

    pub async fn notify_tools_changed(&self) {
        let peers = self.peers.read().await.clone();

        for peer in peers {
            if let Err(error) = peer.notify_tool_list_changed().await {
                warn!("notify tools/list_changed failed: {error}");
            }
        }
    }

    async fn current_config(&self) -> Arc<ToolsConfig> {
        self.config.read().await.clone()
    }

    async fn add_peer(&self, peer: Peer<RoleServer>) {
        self.peers.write().await.push(peer);
    }
}

#[derive(Clone)]
pub struct DynamicTools {
    state: Arc<HotToolsState>,
}

impl DynamicTools {
    pub fn new(state: Arc<HotToolsState>) -> Self {
        Self { state }
    }
}

impl ServerHandler for DynamicTools {
    fn get_info(&self) -> ServerInfo {
        let mut capabilities = ServerCapabilities::default();
        capabilities.tools = Some(ToolsCapability {
            list_changed: Some(true),
        });

        ServerInfo::new(capabilities)
            .with_protocol_version(ProtocolVersion::V_2025_11_25)
            .with_server_info(Implementation::new("db-mcp-tools", "0.1.0"))
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, ErrorData>> + MaybeSendFuture + '_
    {
        async move {
            let config = self.state.current_config().await;
            let tool = config.find_tool(request.name.as_ref()).ok_or_else(|| {
                ErrorData::invalid_params(format!("tool not found: {}", request.name), None)
            })?;

            let args = request.arguments.unwrap_or_default();

            let result = self
                .state
                .database_executor
                .execute(tool.sql(), &args)
                .await
                .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
            let contents = vec![Content::text(result.to_string())];
            Ok(CallToolResult::success(contents))
        }
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, ErrorData>> + MaybeSendFuture + '_
    {
        async move {
            let config = self.state.current_config().await;
            Ok(ListToolsResult {
                tools: config
                    .tools()
                    .iter()
                    .map(|tool| tool.to_mcp_tool())
                    .collect(),
                ..Default::default()
            })
        }
    }

    fn get_tool(&self, name: &str) -> Option<rmcp::model::Tool> {
        let config = self.state.config.try_read().ok()?;
        config.find_tool(name).map(|tool| tool.to_mcp_tool())
    }

    fn on_initialized(
        &self,
        ctx: NotificationContext<RoleServer>,
    ) -> impl std::future::Future<Output = ()> + MaybeSendFuture + '_ {
        async move {
            self.state.add_peer(ctx.peer.clone()).await;
        }
    }

    async fn ping(&self, _ctx: RequestContext<RoleServer>) -> Result<(), ErrorData> {
        Ok(())
    }
}
