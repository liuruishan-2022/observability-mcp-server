use std::sync::Arc;

use crate::config::tool::ToolsConfig;
use rmcp::model::{CallToolResult, Content};
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    handler::server::tool::{ToolCallContext, ToolRoute, ToolRouter},
    model::{
        Implementation, ProtocolVersion, ServerCapabilities, ServerInfo, ToolsCapability,
    },
    service::{NotificationContext, RequestContext},
    tool_handler,
};

///
/// 使用动态的tools做自定义的配置形式
///

#[derive(Clone)]
pub struct DynamicTools {
    tool_router: ToolRouter<Self>,
}

impl DynamicTools {
    pub fn new(config: Arc<ToolsConfig>) -> Self {
        let mut router = ToolRouter::new();
        config
            .mcp_tools()
            .into_iter()
            .map(|tool| {
                ToolRoute::new_dyn(tool, move |ctx: ToolCallContext<'_, Self>| {
                    Box::pin(async move {
                        let args = ctx.arguments.expect("获取参数失败");
                        let contents = vec![Content::text(format!(
                            "收到请求动态的Mcp Tool请求:{}",
                            serde_json::Value::Object(args)
                        ))];
                        Ok(CallToolResult::success(contents))
                    })
                })
            })
            .for_each(|route| {
                router.add_route(route);
            });

        Self {
            tool_router: router,
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for DynamicTools {
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
