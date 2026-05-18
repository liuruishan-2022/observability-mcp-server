# 用 Rust 写一套真实落地的 MCP Tools：从可观测性到研发协同

> 本文基于 `observability-mcp-tools` 项目的实现整理。它不是一个 MCP Demo，而是一套面向真实基础设施、可观测性和研发协同场景的 Rust MCP Tools 工程实践。

## 一、为什么 MCP Tools 不能只停留在 Demo

很多 MCP Server 的示例都很轻：定义一个 `hello` 工具、返回一段字符串、让大模型可以调用。这个阶段适合理解协议，但离真实落地还差几层距离。

在生产环境里，MCP Tools 面对的不是单一函数调用，而是各种内部系统：

- Prometheus：查指标、查告警、查规则、看 TSDB 状态；
- Loki：查日志、查标签、查 LogQL 结果；
- Harbor：管理项目、仓库、镜像制品和 Helm Chart；
- Nacos：读取服务发现、实例、订阅者和配置中心数据；
- Kafka：创建 Topic、生产消息、消费消息、描述 Topic；
- Doris：查元数据、执行 SQL、分析慢查询、看资源增长；
- Jira、Confluence、Bitbucket：把研发协同系统接入 AI 工作流；
- Kubernetes：列 Pod、查事件、看 kubeconfig、做基础运维动作；
- 企业微信：把工具调用结果推送到群机器人。

这类场景的核心问题不是“如何暴露一个函数”，而是如何把企业内部复杂系统，封装成稳定、可控、可被大模型安全调用的工具接口。

`observability-mcp-tools` 的价值就在这里：它用 Rust 把多个真实系统接入 MCP，让 AI 客户端可以通过统一协议完成观测、排障、运维和协作动作。

## 二、整体架构：MCP 层和系统客户端层解耦

项目位于 workspace 下的 `observability-mcp-tools` crate：

```text
observability-mcp-tools/
├── src/
│   ├── main.rs              # HTTP MCP Server 入口
│   ├── mcp/
│   │   ├── mod.rs
│   │   └── tools.rs         # MCP Tool 入参、注册和处理函数
│   ├── searcher/
│   │   ├── mod.rs           # 客户端聚合、环境变量、共享 HTTP 配置
│   │   ├── prometheus.rs
│   │   ├── loki.rs
│   │   ├── harbor.rs
│   │   ├── nacos.rs
│   │   ├── kafka.rs
│   │   ├── doris.rs
│   │   ├── kubernetes.rs
│   │   ├── atlassian.rs
│   │   └── weixin.rs
│   └── docs/
│       └── mod.rs           # 文档加载与搜索
└── Cargo.toml
```

从调用链看，可以分成四层：

```text
AI Client
   |
   | MCP over HTTP
   v
Axum + rmcp Streamable HTTP Server
   |
   v
mcp::tools::Tools
   |
   v
searcher::*Client
   |
   v
Prometheus / Loki / Harbor / Nacos / Kafka / Doris / Jira / Confluence / Bitbucket / Kubernetes / Weixin
```

这个分层非常关键。

MCP 层只负责三件事：声明工具、校验参数、把调用转发给后端客户端。真正的系统 API 细节放在 `searcher` 层，例如 Prometheus API、Kubernetes API、企业微信 Webhook、Atlassian REST API 等。

这样做的好处是：当后端系统 API 变化时，主要修改客户端实现；当 MCP 协议或工具描述变化时，主要修改 `mcp/tools.rs`。两层职责清晰，后续扩展工具族也比较自然。

## 三、入口设计：一个 Streamable HTTP MCP Server

服务入口在 `src/main.rs`，核心逻辑非常直接：

```rust
let service = StreamableHttpService::new(
    || Ok(Tools::new()),
    LocalSessionManager::default().into(),
    server_config,
);

let router = axum::Router::new().nest_service("/mcp", service);
let tcp_listener = tokio::net::TcpListener::bind("0.0.0.0:3013").await?;
axum::serve(tcp_listener, router).await?;
```

这里有几个工程化细节值得注意。

第一，服务使用 `rmcp` 的 `StreamableHttpService`。这意味着 MCP Server 不是本地 stdio 工具，而是一个可远程访问的 HTTP 服务。对企业内部使用来说，这更适合部署在容器、Kubernetes 或运维网段里。

第二，入口通过 `Tools::new()` 创建工具集合。每个 MCP 会话拿到一个工具处理器，工具处理器内部再聚合不同系统的客户端。

第三，服务监听 `0.0.0.0:3013`，并把 MCP endpoint 暴露在 `/mcp`。这让部署侧可以很容易用 Nginx、Ingress、ServiceMesh 或内网网关做转发。

第四，项目使用 `tokio_util::sync::CancellationToken` 配合 `ctrl_c` 做优雅退出。对于一个长时间运行的 MCP Server 来说，这比简单粗暴退出更适合生产环境。

## 四、Tools 聚合：把复杂系统变成模型可调用的函数

`mcp/tools.rs` 是整个项目的工具声明中心。项目使用 `rmcp` 的宏来声明工具路由：

```rust
#[tool_router(router = tool_router)]
impl Tools {
    #[tool(description = "执行PromQL即时查询")]
    pub async fn prom_query(&self, Parameters(params): Parameters<QueryRequest>) -> String {
        // ...
    }
}
```

一个工具通常由三部分组成。

第一部分是入参结构体：

```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct QueryRequest {
    #[schemars(description = "PromQL 查询语句")]
    query: String,
    #[schemars(description = "可选的时间戳，Unix时间戳或RFC3339格式")]
    time: Option<String>,
}
```

这里同时派生了 `Serialize`、`Deserialize` 和 `JsonSchema`。这不是为了 Rust 自己看，而是为了让 MCP 客户端和模型理解参数结构：字段名是什么、类型是什么、每个字段代表什么。

第二部分是工具函数：

```rust
#[tool(description = "执行PromQL即时查询")]
pub async fn prom_query(&self, Parameters(params): Parameters<QueryRequest>) -> String {
    match self.searcher.prometheus.query(&params.query, params.time.as_deref()).await {
        Ok(result) => serde_json::to_string(&result).unwrap_or_else(|e| e.to_string()),
        Err(e) => e.to_string(),
    }
}
```

工具函数不直接拼 HTTP 请求，而是委托给 `self.searcher.prometheus`。这能保证 MCP 层足够薄，避免工具函数变成大量业务协议细节的堆积。

第三部分是服务能力声明：

```rust
impl ServerHandler for Tools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            // ...
        }
    }
}
```

也就是说，这个服务对外明确声明：我是一台 MCP Server，并且支持 tools 能力。

## 五、Searcher 层：真实系统客户端的统一入口

项目把所有外部系统客户端收敛到 `Searcher`：

```rust
pub struct Searcher {
    pub prometheus: PrometheusClient,
    pub loki: LokiClient,
    pub harbor: Option<HarborClient>,
    pub nacos: Option<NacosClient>,
    pub kafka: Option<KafkaClient>,
    pub doris: Option<DorisClient>,
    pub weixin: Option<WeixinClient>,
    pub jira: Option<JiraClient>,
    pub confluence: Option<ConfluenceClient>,
    pub bitbucket: Option<BitbucketClient>,
    pub kubernetes: Option<KubernetesClient>,
}
```

这里有一个很实用的设计：Prometheus 和 Loki 是必选能力，其他系统大多是可选能力。

原因很简单。不同团队内部系统不一样，有的有 Harbor，有的没有；有的用 Jira，有的用其他项目管理系统；有的环境允许 MCP Server 访问 Kubernetes，有的环境严格禁止。通过 `Option<Client>` 表示可选能力，可以让同一个二进制适配更多部署环境。

构建客户端时，项目从环境变量读取配置：

```rust
let prometheus_root = var("PROMETHEUS_ROOT")
    .map_err(|_| SearcherError::EnvVarNotSet("PROMETHEUS_ROOT".to_string()))?;

let harbor = if let (Ok(url), Ok(username), Ok(password)) = (
    var("HARBOR_URL"),
    var("HARBOR_USERNAME"),
    var("HARBOR_PASSWORD"),
) {
    Some(HarborClient::new(url, username, password))
} else {
    None
};
```

这种方式对容器化部署很友好：配置不写死在代码里，不需要为不同环境重新编译，通过 `.env`、Kubernetes Secret 或 CI/CD 注入即可。

## 六、HTTP 客户端：统一超时、证书和错误信息

真实环境里，MCP Tool 最常见的问题不是代码编译不过，而是“工具调用失败后不知道为什么失败”。

项目在 `searcher/mod.rs` 里抽了共享 HTTP Client：

```rust
const DEFAULT_HTTP_SSL_VERIFY: bool = false;
const DEFAULT_HTTP_CONNECT_TIMEOUT_SECS: u64 = 10;
const DEFAULT_HTTP_TIMEOUT_SECS: u64 = 30;

fn shared_http_client_builder(ssl_verify: bool) -> ClientBuilder {
    Client::builder()
        .danger_accept_invalid_certs(!ssl_verify)
        .connect_timeout(Duration::from_secs(DEFAULT_HTTP_CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS))
}
```

这体现了两个落地经验。

第一，内网系统经常使用自签证书。`HTTP_SSL_VERIFY` 做成全局开关，可以兼容这类环境。当然，如果是互联网暴露或高安全环境，建议开启证书校验。

第二，所有 HTTP 请求都必须有超时。MCP Tool 一旦被模型调用，如果底层请求无限等待，上层用户体验会非常差，也容易拖垮服务资源。

项目还对 `reqwest::Error` 做了详细格式化：

```rust
if let Some(url) = error.url() {
    details.push(format!("url={url}"));
}
if let Some(status) = error.status() {
    details.push(format!("status={status}"));
}
if error.is_timeout() {
    details.push("kind=timeout".to_string());
}
```

这样工具调用失败时，返回的不只是 `request failed`，而是能看到 URL、HTTP 状态码、是否超时、是否连接失败、底层 cause 链。对排障来说，这个细节非常值钱。

## 七、工具族设计：不是所有接口都适合暴露给模型

这个项目覆盖的工具很多，但它们并不是简单把后端系统 API 全量复制出来，而是围绕常见工作流组织。

### 1. 可观测性工具

Prometheus 工具覆盖了查询、元数据、告警和系统状态：

- `prom_query`、`prom_query_range`：执行 PromQL；
- `prom_alerts`、`prom_rules`、`prom_targets`：排查告警和采集状态；
- `prom_tsdb_status`、`prom_wal_replay`：定位存储和启动恢复问题；
- `prom_labels`、`prom_label_values`、`prom_metadata`：辅助模型理解指标空间。

Loki 工具则聚焦日志查询：

- `loki_query`：执行 LogQL；
- `loki_labels`、`loki_label_values`：帮助模型发现日志标签。

这类工具适合让 AI 做一线排障助手。例如用户问：“为什么订单服务 10 分钟内错误率上升？”模型可以先查 Prometheus，再根据 service 标签去 Loki 查日志。

### 2. 基础设施工具

Harbor、Nacos、Kafka、Kubernetes 属于基础设施控制面。

这些工具的风险比只读查询更高，因为它们可能包含创建、删除、重载等操作。例如：

- 删除 Harbor artifact；
- 删除 Kafka topic；
- 删除 Kubernetes Pod；
- 重载 Prometheus 配置。

在真实落地时，这类工具建议配合网关鉴权、审计日志、只读/写入分组、二次确认或环境隔离使用。MCP 能让模型调用工具，但生产系统仍然需要明确的权限边界。

### 3. 数据平台工具

Doris 工具不仅做基础元数据查询，还提供了更接近数据治理的能力：

- 表结构、表注释、列注释；
- SQL 执行计划和 Profile；
- 慢查询 TopN；
- 表存储、分桶、分区分析；
- 数据新鲜度、列完整性、访问模式分析。

这类工具很适合把 AI 从“会写 SQL”推进到“能辅助数据平台排障和治理”。模型不只是生成查询，还能结合元数据和运行状态给出诊断建议。

### 4. 研发协同工具

Jira、Confluence、Bitbucket 工具把研发流程也接了进来。

这意味着 AI 可以完成更长链路的工作：查告警、定位服务、找相关代码仓库、看最近 PR、检索 Confluence 文档、最后在 Jira 里补充评论或创建 issue。

如果只接 Prometheus，AI 只能回答“指标发生了什么”；接入研发协同系统后，AI 才有机会回答“这个问题可能是谁改的、文档在哪里、后续应该跟进什么”。

### 5. 通知工具

企业微信工具负责把结果推送出去：

- 文本消息；
- Markdown 消息；
- 图片、文件、图文消息。

这类工具通常不是排障的第一步，但很适合作为工作流最后一步：把诊断结论、巡检日报、慢查询报告、发布风险提示推送到群里。

## 八、为什么 Rust 适合写这类 MCP Server

MCP Server 本质上是一个长期运行的工具网关。它需要同时面对网络 IO、JSON 序列化、鉴权信息、错误处理和并发请求。Rust 在这个场景里的优势比较明确。

第一，异步 IO 成熟。项目基于 `tokio`、`axum`、`reqwest`，可以自然处理大量外部系统调用。

第二，类型系统能约束工具入参。每个 MCP Tool 都有明确的 request struct，并通过 `JsonSchema` 暴露给上层。相比动态语言里到处传 `map`，这种方式更容易维护。

第三，错误边界清晰。`thiserror` 定义统一错误类型，`anyhow` 处理入口级错误，工具函数最终把错误转换成模型可读字符串。

第四，部署简单。编译后是一个二进制，容器镜像可以做得很薄，适合放在内网工具平台或 Kubernetes 里长期运行。

## 九、如何新增一个真实 MCP Tool

基于当前项目，新增一个工具通常走五步。

### 第一步：在 searcher 层实现系统客户端方法

例如要给某个系统新增 `get_status`：

```rust
impl SomeClient {
    pub async fn get_status(&self) -> Result<SomeStatus, SearcherError> {
        let resp = self.client
            .get(format!("{}/api/status", self.base_url))
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(resp.json::<SomeStatus>().await?)
        } else {
            Err(SearcherError::ApiError(format!(
                "status api failed: {}",
                resp.status()
            )))
        }
    }
}
```

这个方法应该只关心后端系统 API，不要混入 MCP 协议逻辑。

### 第二步：定义 MCP 入参结构

```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct SomeStatusRequest {
    #[schemars(description = "环境名称，例如 prod 或 staging")]
    env: String,
}
```

字段描述要写给模型看。描述越清楚，模型越不容易传错参数。

### 第三步：在 `Tools` 里注册工具函数

```rust
#[tool(description = "获取指定环境的系统状态")]
pub async fn some_get_status(
    &self,
    Parameters(params): Parameters<SomeStatusRequest>,
) -> String {
    match self.searcher.some_client.get_status(&params.env).await {
        Ok(result) => serde_json::to_string(&result).unwrap_or_else(|e| e.to_string()),
        Err(e) => e.to_string(),
    }
}
```

注意工具返回值目前统一是 `String`。成功时返回 JSON 字符串，失败时返回错误字符串。这种方式简单直接，适合被大模型消费。

### 第四步：从环境变量构建客户端

在 `build_searcher` 或 `build_searcher_async` 里读取配置：

```rust
let some_client = if let Ok(base_url) = var("SOME_SYSTEM_URL") {
    Some(SomeClient::new(base_url))
} else {
    None
};
```

如果工具依赖可选系统，调用时要处理未配置场景，返回明确错误，例如 `SOME_SYSTEM_URL is not configured`。

### 第五步：补充运行文档和最小验证

至少需要说明：

- 需要哪些环境变量；
- 工具是只读还是写入；
- 调用示例是什么；
- 失败时如何排查；
- 是否需要额外权限。

真实落地的 MCP Tool，文档不是锦上添花，而是让团队敢用的前提。

## 十、生产化落地建议

基于这个项目继续推进到生产环境，建议重点补齐几类能力。

### 1. 工具权限分级

把工具分成只读、写入、高危三类。

只读工具如查询指标、查询日志、读取文档，可以开放给更广泛的 AI 客户端。写入工具如创建 issue、发送企业微信消息，需要用户身份和审计。高危工具如删除镜像、删除 Pod、删除 Topic，建议默认关闭，或只在受控环境中启用。

### 2. 审计日志

每一次工具调用都应该记录：

- 调用者；
- 工具名；
- 参数摘要；
- 调用时间；
- 目标系统；
- 成功或失败；
- 失败原因。

MCP Server 会成为 AI 和内部系统之间的入口，没有审计就很难回溯问题。

### 3. 参数白名单和限流

对 PromQL、LogQL、SQL、JQL 这类查询语言，要考虑限制查询范围、默认时间窗口和最大返回条数。

对写入类操作，要限制目标 namespace、project、topic、repository 或 issue project，避免模型误操作到不该碰的环境。

### 4. 返回结果裁剪

后端系统的原始返回可能很大。直接把完整 JSON 返回给模型，会带来上下文浪费，也可能泄露不必要的信息。

更好的方式是对结果做摘要：保留关键字段、分页返回、按错误和风险排序、对大字段截断。

### 5. 运行时可观测性

MCP Server 自己也应该被观测。至少要有：

- 请求耗时；
- 工具调用次数；
- 工具失败率；
- 下游系统错误率；
- 超时次数；
- 当前版本和构建信息。

当 AI 工具平台自身出问题时，团队也需要能快速定位。

## 十一、总结

`observability-mcp-tools` 展示了一种比较务实的 MCP 落地方式：用 Rust 写一个稳定的远程 MCP Server，把企业内部的可观测性、基础设施、数据平台和研发协同系统统一包装成 tools。

它的关键不是某一个 API，而是工程模型：

- MCP 层负责工具声明和参数 schema；
- searcher 层负责真实系统客户端；
- 环境变量负责部署配置；
- 共享 HTTP Client 负责超时、TLS 和错误细节；
- 可选客户端负责适配不同企业环境；
- 工具族围绕真实运维和研发工作流组织。

如果把 MCP 看成 AI 应用连接真实世界的协议，那么这类项目就是“协议到生产系统”的最后一公里。Demo 能证明 MCP 可以调用函数，而真实落地的 MCP Tools 要证明：它能稳定、安全、可维护地调用企业内部系统。
