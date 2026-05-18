use rmcp::model::{CallToolResult, Content};
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    handler::server::tool::{ToolCallContext, ToolRoute, ToolRouter},
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo, Tool, ToolsCapability},
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
    pub fn new() -> Self {
        let mut router = ToolRouter::new();
        let schema = rmcp::model::object(serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }));

        let tool = Tool::new(
            "db数据库工具mcp",
            "可以动态的设置tools,根据配置进行加载",
            schema,
        );

        let route = ToolRoute::new_dyn(tool, move |_ctx: ToolCallContext<'_, Self>| {
            Box::pin(async move {
                Ok(CallToolResult::success(vec![Content::text(
                    "这是一个动态的MCP Tool返回的固定的字符串",
                )]))
            })
        });

        router.add_route(route);

        let echo_schema = rmcp::model::object(serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "需要回显的名称"
                },
                "repeat": {
                    "type": "integer",
                    "description": "重复次数，默认 1",
                    "minimum": 1
                }
            },
            "required": ["name"],
            "additionalProperties": false
        }));

        let echo_tool = Tool::new(
            "echo_name",
            "接收 name 和 repeat 参数，并返回拼接后的字符串",
            echo_schema,
        );

        let echo_route = ToolRoute::new_dyn(echo_tool, move |ctx: ToolCallContext<'_, Self>| {
            Box::pin(async move {
                let args = ctx.arguments.unwrap_or_default();

                let name = args.get("name").and_then(|value| value.as_str()).ok_or_else(|| {
                    ErrorData::invalid_params("missing required parameter: name", None)
                })?;

                let repeat = args
                    .get("repeat")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(1);

                let text = (0..repeat)
                    .map(|_| name)
                    .collect::<Vec<_>>()
                    .join(", ");

                Ok(CallToolResult::success(vec![Content::text(text)]))
            })
        });

        router.add_route(echo_route);

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
