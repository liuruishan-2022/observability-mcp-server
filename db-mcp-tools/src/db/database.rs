use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    handler::server::tool::ToolRouter,
    model::{
        CallToolResult, Content, Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
        ToolsCapability,
    },
    service::{NotificationContext, RequestContext},
    tool, tool_handler, tool_router,
};

#[derive(Clone)]
pub struct DatabaseExecutor {
    tool_router: ToolRouter<DatabaseExecutor>,
}

#[tool_router(router=tool_router)]
impl DatabaseExecutor {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "获取数据库的名字")]
    async fn name(&self) -> Result<CallToolResult, ErrorData> {
        let contents = vec![Content::text("database的Name信息获取".to_string())];
        return Ok(CallToolResult::success(contents));
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DatabaseExecutor {
    fn get_info(&self) -> ServerInfo {
        let mut capabilities = ServerCapabilities::default();
        capabilities.tools = Some(ToolsCapability {
            list_changed: Some(false),
        });

        ServerInfo::new(capabilities)
            .with_protocol_version(ProtocolVersion::V_2025_11_25)
            .with_server_info(Implementation::new("db-mcp-tools", "0.1.0"))
    }

    async fn ping(&self, _ctx: RequestContext<RoleServer>) -> Result<(), ErrorData> {
        Ok(())
    }

    async fn on_initialized(&self, _ctx: NotificationContext<RoleServer>) {}
}
