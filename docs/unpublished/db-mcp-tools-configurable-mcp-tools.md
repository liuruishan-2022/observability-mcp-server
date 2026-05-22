# db-mcp-tools 配置化 MCP Tools 技术设计

## 1. 需求背景

运维同事希望有一组用于数据库查询的 MCP tools，让 Agent 能够查询业务数据，但不希望 Agent 直接传递 SQL。

直接让 Agent 传 SQL 的问题是：即使可以通过 MySQL 账号权限限制删除、修改、新增等高风险操作，也很难控制字段级别的数据访问。例如同一张表里可能有部分字段可以查询，部分字段不应该暴露给 Agent。仅靠数据库账号很难表达这种细粒度约束。

同时，实际运维场景中使用的 SQL 大多是固定的，并且每条 SQL 都有明确的业务含义。例如：

- 查询某个账户的余额。
- 查询某个用户的发送成功率。
- 查询某个消息 ID 的发送上下文。
- 统计某类任务的总量。

这类查询并不需要 Agent 自由生成 SQL，更适合由服务端预先定义好 SQL，Agent 只传必要参数。

基于这个需求，有两种实现方案。

第一种方案是静态 tool：每条 SQL 都像传统 MCP tool 一样，在 Rust 代码里写一个固定方法。每个 tool 的逻辑基本类似：接收参数、执行 SQL、返回结果。这种方式清晰、安全，但问题是每次运维同事要新增或修改一个查询，都需要修改代码、重新构建并发布程序。

第二种方案是配置化 tool：把 tool 名称、描述、参数 schema 和 SQL 都放到 YAML 配置中。服务启动时读取配置，自动生成 MCP tools。这样 SQL 仍然由服务端控制，Agent 不能直接传任意 SQL；同时运维侧新增或调整固定查询时，主要修改配置文件。

`db-mcp-tools` 当前采用第二种方案：配置化 MCP tools。

## 2. 背景和目标

`db-mcp-tools` 的目标是把数据库查询能力配置化：通过 YAML 文件声明一组 SQL 查询工具，服务启动时读取配置，并把这些配置转换成 MCP tools。这样新增查询能力时，主要修改配置文件，而不是为每个查询都手写一个 Rust 方法。

这份文档重点说明两件事：

1. 如何不使用 `#[tool]` 宏，而是通过代码手动组合 `Tool`、`ToolRoute`、`ToolRouter`。
2. 当前项目如何基于 YAML 配置生成 MCP tools，并执行配置中的 SQL。

当前版本只支持“启动时配置化生成 MCP tools”。暂时不支持运行中监听 `db-tools.yaml` 并动态增删改 tools。

## 3. 为什么要手动组合 Tool Route Router

rmcp 提供了宏方式定义 tool，例如 `#[tool]`。宏适合静态工具：工具名称、参数、处理逻辑在编译期基本固定。

但是本项目的目标是配置化：

- tool 名称来自 YAML。
- tool 描述来自 YAML。
- tool 参数 schema 来自 YAML。
- tool 执行 SQL 来自 YAML。

这些内容不是写死在 Rust 函数上的，所以更适合手动构造：

```text
YAML 配置
    -> rmcp::model::Tool
    -> ToolRoute::new_dyn(tool, handler)
    -> ToolRouter::add_route(route)
```

这个能力也是以后做运行时动态 tools 的基础。只有先理解如何手动创建 route，后面才能讨论 reload 后增删改 route。

## 4. rmcp 中的三个核心对象

### 4.1 Tool

`Tool` 是暴露给 MCP client 的工具元数据，主要包括：

- `name`: tool 名称。
- `description`: tool 描述。
- `inputSchema`: tool 参数 schema。

手动创建一个最小 `Tool`：

```rust
use rmcp::model::Tool;

let schema = rmcp::model::object(serde_json::json!({
    "type": "object",
    "properties": {
        "id": {
            "type": "integer",
            "description": "主键 ID"
        }
    },
    "required": ["id"],
    "additionalProperties": false
}));

let tool = Tool::new(
    "get_user_by_id",
    "根据 ID 查询用户",
    schema,
);
```

### 4.2 ToolRoute

`ToolRoute` 是 tool 元数据和执行逻辑的绑定。

```rust
use rmcp::handler::server::tool::{ToolCallContext, ToolRoute};
use rmcp::model::{CallToolResult, Content};

let route = ToolRoute::new_dyn(
    tool,
    move |ctx: ToolCallContext<'_, DynamicTools>| {
        Box::pin(async move {
            let args = ctx.arguments.unwrap_or_default();

            Ok(CallToolResult::success(vec![Content::text(format!(
                "收到参数: {}",
                serde_json::Value::Object(args)
            ))]))
        })
    },
);
```

这里注意：

- `ToolRoute::new_dyn` 会拿走 `tool` 的所有权。
- handler 是一个闭包，MCP client 调用 tool 时由 rmcp 调用。
- `ctx.arguments` 是 client 调用 tool 时传入的 JSON 参数。

### 4.3 ToolRouter

`ToolRouter` 保存多个 route。

```rust
use rmcp::handler::server::tool::ToolRouter;

let mut router = ToolRouter::new();
router.add_route(route);
```

后续 MCP 请求进来时，rmcp 会根据 tool name 从 router 中找到对应 route 并执行。

## 5. 不使用宏创建一个完整动态 Tool

下面是一个完整的手动组合例子。这个例子没有用 `#[tool]` 宏。

```rust
use rmcp::handler::server::tool::{ToolCallContext, ToolRoute, ToolRouter};
use rmcp::model::{CallToolResult, Content, Tool};

#[derive(Clone)]
pub struct DynamicTools {
    tool_router: ToolRouter<Self>,
}

impl DynamicTools {
    pub fn new() -> Self {
        let mut router = ToolRouter::new();

        let schema = rmcp::model::object(serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "需要回显的名称"
                }
            },
            "required": ["name"],
            "additionalProperties": false
        }));

        let tool = Tool::new(
            "echo_name",
            "回显 name 参数",
            schema,
        );

        let route = ToolRoute::new_dyn(
            tool,
            move |ctx: ToolCallContext<'_, Self>| {
                Box::pin(async move {
                    let args = ctx.arguments.unwrap_or_default();
                    let name = args
                        .get("name")
                        .and_then(|value| value.as_str())
                        .unwrap_or("");

                    Ok(CallToolResult::success(vec![Content::text(name.to_string())]))
                })
            },
        );

        router.add_route(route);

        Self {
            tool_router: router,
        }
    }
}
```

这个例子是本项目配置化实现的基础。区别只是：真实项目里 `Tool` 不是写死的，而是从 YAML 配置转换出来的。

## 6. 当前项目的配置模型

配置结构定义在 `db-mcp-tools/src/config/tool.rs`。

```rust
type McpJsonObject = rmcp::model::JsonObject;
type McpTool = rmcp::model::Tool;
type Properties = HashMap<String, FieldProperty>;

#[derive(Serialize, Deserialize)]
pub struct ToolsConfig {
    tools: Vec<Tool>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Tool {
    name: String,
    description: String,
    sql: String,
    properties: Properties,
    required: Vec<String>,
}
```

一个 YAML tool 对应一个 `Tool` 配置。字段含义：

- `name`: MCP tool 名称。
- `description`: MCP tool 描述。
- `sql`: 服务端执行的 SQL。
- `properties`: MCP 参数 schema 的 properties 部分。
- `required`: 必填参数列表。

示例配置：

```yaml
tools:
  - name: get_bytedance_video_message_context_by_id
    description: 根据主键 ID 查询字节视频短信发送上下文单条记录
    sql: |
      SELECT id, user_id, msg_id, mobile, status, create_time
      FROM inter_gsms.bytedance_video_message_context
      WHERE id = #{id}
      LIMIT 1
    properties:
      id:
        type: integer
        description: bytedance_video_message_context 主键 ID
    required:
      - id
```

## 7. 从配置生成 MCP Tool

`Tool::to_mcp_tool` 负责把配置转换成 `rmcp::model::Tool`。

```rust
impl Tool {
    fn to_mcp_tool(&self) -> McpTool {
        let input_schema = InputSchema::new(&self.properties);
        McpTool::new(
            self.name.clone(),
            self.description.clone(),
            input_schema.to_mcp_json_object(),
        )
    }

    pub fn sql(&self) -> &str {
        self.sql.as_str()
    }
}
```

`InputSchema` 负责生成 MCP input schema：

```rust
#[derive(Serialize)]
pub struct InputSchema<'a> {
    #[serde(rename = "type")]
    schema_type: String,
    properties: &'a Properties,
}

impl<'a> InputSchema<'a> {
    pub fn new(properties: &'a Properties) -> Self {
        InputSchema {
            schema_type: "object".to_string(),
            properties,
        }
    }

    fn to_mcp_json_object(&self) -> Arc<McpJsonObject> {
        let json = rmcp::model::object(
            serde_json::to_value(self).expect("serialize to serde json value failed"),
        );
        Arc::new(json)
    }
}
```

当前 schema 主要包含 `type` 和 `properties`。后续如果要更严格兼容 MCP/JSON Schema，可以把 `required` 和 `additionalProperties` 也放入 `InputSchema`。

## 8. 为什么需要 ToolWrapper

注册 `ToolRoute` 时，`ToolRoute::new_dyn(tool, handler)` 会拿走 `tool` 的所有权。

但是执行 SQL 时，我们还需要原始配置里的 `sql`。所以项目引入了 `ToolWrapper`：

```rust
pub struct ToolWrapper {
    tool: McpTool,
    config: Tool,
}

impl ToolWrapper {
    pub fn into_parts(self) -> (McpTool, Tool) {
        (self.tool, self.config)
    }
}
```

`ToolsConfig::mcp_tools()` 会为每个配置生成一个 wrapper：

```rust
impl ToolsConfig {
    pub fn mcp_tools(&self) -> Vec<ToolWrapper> {
        self.tools
            .iter()
            .map(|ele| ToolWrapper {
                tool: ele.to_mcp_tool(),
                config: ele.clone(),
            })
            .collect()
    }
}
```

这样每个 YAML tool 都同时拥有：

- 用于注册 route 的 MCP `Tool`。
- 用于执行 SQL 的原始配置 `Tool`。

## 9. 从 ToolWrapper 生成 ToolRoute

核心代码在 `db-mcp-tools/src/mcp/tools.rs`。

```rust
config
    .mcp_tools()
    .into_iter()
    .map(|wrapper| {
        let (tool, tool_config) = wrapper.into_parts();
        let sql: Arc<str> = Arc::from(tool_config.sql());
        let database_executor = database_executor.clone();

        ToolRoute::new_dyn(tool, move |ctx: ToolCallContext<'_, Self>| {
            let database_executor = database_executor.clone();
            let sql = sql.clone();

            Box::pin(async move {
                let args = ctx.arguments.expect("获取参数失败");

                let result = database_executor
                    .execute(&sql, &args)
                    .await
                    .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;

                Ok(CallToolResult::success(vec![
                    Content::text(result.to_string())
                ]))
            })
        })
    })
    .for_each(|route| {
        router.add_route(route);
    });
```

这里有几个关键点。

第一，`tool` 必须按值传给 `ToolRoute::new_dyn`：

```rust
ToolRoute::new_dyn(tool, ...)
```

因为 route 需要拥有自己的 tool 元数据。

第二，SQL 使用 `Arc<str>`：

```rust
let sql: Arc<str> = Arc::from(tool_config.sql());
```

handler 是 `Fn`，可能被调用多次，不能每次调用时消费同一个 `String`。`Arc<str>` 可以低成本 clone，适合放进闭包。

第三，数据库执行器使用 `Arc<DatabaseExecutor>`：

```rust
let database_executor = database_executor.clone();
```

这样每个 route handler 都能共享同一个连接池执行器。

## 10. SQL 占位符替换

SQL 支持类似 MyBatis 的占位符：

```sql
WHERE id = #{id}
```

MCP client 调用时传入：

```json
{
  "id": 1
}
```

`DatabaseExecutor::render_sql` 会把 SQL 渲染成：

```sql
WHERE id = 1
```

核心函数：

```rust
pub fn render_sql(query: &str, params: &Map<String, Value>) -> Result<String> {
    let mut rendered = String::with_capacity(query.len());
    let mut rest = query;

    while let Some(start) = rest.find("#{") {
        rendered.push_str(&rest[..start]);
        let placeholder = &rest[start + 2..];
        let Some(end) = placeholder.find('}') else {
            bail!("SQL placeholder missing closing brace: {}", rest);
        };

        let name = &placeholder[..end];
        let value = params
            .get(name)
            .ok_or_else(|| anyhow!("missing SQL parameter: {}", name))?;

        rendered.push_str(&Self::value_to_sql_text(value));
        rest = &placeholder[end + 1..];
    }

    rendered.push_str(rest);
    Ok(rendered)
}
```

注意：当前实现是字符串替换，不是 SQL 预编译绑定参数。后续如果用于生产环境，需要重点考虑 SQL 注入风险，建议改造成参数化查询。

## 11. SQL 执行和 JSON 返回

`DatabaseExecutor::execute` 会执行渲染后的 SQL，并把查询结果转成 JSON。

```rust
pub async fn execute(&self, query: &str, params: &Map<String, Value>) -> Result<Value> {
    let sql = Self::render_sql(query, params)?;
    let rows = sqlx::query(sql.as_str()).fetch_all(&self.pool).await?;
    let mut values = Vec::with_capacity(rows.len());

    for row in rows {
        let mut object = Map::new();

        for column in row.columns() {
            let name = column.name();
            let value = Self::mysql_column_to_json(&row, name, column.type_info().name());
            object.insert(name.to_string(), value);
        }

        values.push(Value::Object(object));
    }

    Ok(Value::Array(values))
}
```

返回结果示例：

```json
[
  {
    "id": 1,
    "user_id": "admin@dsapt1",
    "status": 2
  }
]
```

## 12. 启动方式和数据库配置

数据库连接信息当前从环境变量读取：

```bash
export HOST="127.0.0.1"
export PORT="3306"
export DATABASE="inter_gsms"
export USERNAME="liuxu"
export PASSWORD="liuxu@110"
```

本地启动脚本：

```bash
./db-mcp-tools/start.sh
```

脚本内部执行：

```bash
cargo run -p db-mcp-tools -- -c ./db-mcp-tools/db-tools.yaml
```

其中 `--` 很重要，它表示后面的 `-c` 是传给 `db-mcp-tools` 程序，而不是传给 `cargo run`。

## 13. 当前不支持运行时动态更新

当前实现是在服务启动时：

```text
读取配置 -> 构造 ToolRoute -> add_route 到 ToolRouter
```

注册完成后，`ToolRouter` 中的 tool 列表是固定的。因此当前版本暂不支持：

- 修改 `db-tools.yaml` 后自动新增 tool。
- 修改 `db-tools.yaml` 后自动删除 tool。
- 修改 `db-tools.yaml` 后自动重建已有 tool 的 schema。
- 配置变化后主动通知 MCP client 重新拉取 tools/list。

如果部署在 Kubernetes 中，当前推荐做法是：

```text
ConfigMap 挂载 db-tools.yaml
    -> ConfigMap 更新
    -> Deployment 滚动重启 Pod
    -> 服务重新读取配置并注册 tools
```

## 14. 后续热更新设计方向

后续如果要支持运行时动态增删改，可以基于两种方向演进。

### 14.1 共享配置，动态 list/call

把配置放到共享状态中：

```rust
Arc<RwLock<ToolsConfig>>
```

后台任务定时扫描 `db-tools.yaml`，发现变更后替换内存配置。

调用时不再依赖启动时捕获的 SQL，而是根据 tool name 从最新配置中查找 SQL。

这个方案适合真正动态的 MCP tools，但需要自己处理 tools/list 和 tools/call 的动态行为。

### 14.2 共享 Router，reload 时增删改 route

`ToolRouter` 本身有这些能力：

```rust
router.add_route(route);
router.remove_route(name);
router.disable_route(name);
router.enable_route(name);
router.has_route(name);
```

理论上可以在配置变更后 diff old/new tools，然后对 router 做 add/remove。

但当前代码使用 `#[tool_handler(router = self.tool_router)]`，router 是 `DynamicTools` 实例内的普通字段。后台 reload 任务无法直接修改这个字段。因此如果采用这个方向，需要重新设计 router 的共享和通知机制。

无论采用哪种热更新方案，如果希望 MCP client 感知 tools 变化，都需要服务端发送 `notifications/tools/list_changed`，并且客户端支持收到通知后重新调用 tools/list。

## 15. 当前结论

当前版本已经完成：

- 配置化声明 MCP tools。
- 启动时根据配置生成 `Tool`、`ToolRoute`、`ToolRouter`。
- tool 调用时执行配置中的 SQL。
- SQL 查询结果以 JSON 返回。

当前版本尚未完成：

- 运行时监听配置文件。
- 运行时动态增删改 MCP tools。
- tools 变化后通知 MCP client 刷新 tools/list。
