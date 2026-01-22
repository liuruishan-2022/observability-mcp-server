# 使用 Rust 编写 Remote MCP Server 完全指南

本指南基于 `observability-mcp-server` 项目，详细介绍如何使用 Rust 编写一个远程 Model Context Protocol (MCP) 服务器。

## 目录

1. [什么是 MCP Server](#什么是-mcp-server)
2. [项目架构概览](#项目架构概览)
3. [环境准备](#环境准备)
4. [项目初始化](#项目初始化)
5. [核心模块实现](#核心模块实现)
6. [定义 API 客户端](#定义-api-客户端)
7. [实现 MCP Tools](#实现-mcp-tools)
8. [配置服务器](#配置服务器)
9. [构建与运行](#构建与运行)
10. [最佳实践](#最佳实践)

---

## 什么是 MCP Server

Model Context Protocol (MCP) 是一个开放协议，允许 AI 应用（如 Claude）与外部工具和数据源进行交互。Remote MCP Server 通过 HTTP/WebSocket 协议提供 MCP 接口。

**核心优势:**
- 🚀 Rust 的高性能和内存安全
- 📡 通过 HTTP 远程访问
- 🛠️ 统一的工具接口
- 🔄 支持 Server-Sent Events (SSE)

---

## 项目架构概览

```
observability-mcp-server/
├── src/
│   ├── main.rs              # 入口文件，配置 HTTP 服务器
│   ├── searcher/            # API 客户端实现
│   │   ├── mod.rs           # 模块导出和错误定义
│   │   ├── prometheus.rs    # Prometheus 客户端
│   │   └── loki.rs          # Loki 客户端
│   ├── mcp/                 # MCP 相关实现
│   │   ├── mod.rs           # 模块导出
│   │   └── tools.rs         # Tool 定义和处理
│   └── docs/                # 文档加载器
└── Cargo.toml               # 项目配置
```

**架构层次:**

```
┌─────────────────────────────────────────────┐
│         HTTP Server (Axum)                  │
│         /mcp endpoint                       │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────▼───────────────────────────┐
│      MCP Service (rmcp)                     │
│      - Protocol handling                    │
│      - Tool routing                         │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────▼───────────────────────────┐
│      Tools Implementation                   │
│      - Tool definitions                     │
│      - Request/Response handlers            │
└─────────────────┬───────────────────────────┘
                  │
┌─────────────────▼───────────────────────────┐
│      API Clients (Searcher)                 │
│      - Prometheus client                    │
│      - Loki client                          │
└─────────────────────────────────────────────┘
```

---

## 环境准备

### 1. 安装 Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. 创建新项目

```bash
cargo new my-mcp-server
cd my-mcp-server
```

### 3. 配置依赖

编辑 `Cargo.toml`:

```toml
[package]
name = "my-mcp-server"
version = "0.1.0"
edition = "2021"

[dependencies]
# 异步运行时
tokio = { version = "1.48", features = ["full"] }

# 错误处理
anyhow = "1.0"
thiserror = "2.0"

# 序列化
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# HTTP 服务器
axum = { version = "0.8", features = ["macros"] }

# HTTP 客户端
reqwest = { version = "0.12", features = ["json"] }

# 日志
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# MCP 核心库
rmcp = { version = "0.13", features = [
    "server",
    "macros",
    "transport-io",
    "transport-streamable-http-server",
] }

# JSON Schema 生成
schemars = "1.2"

# Tokio 工具
tokio-util = "0.7"

# 环境变量
dotenv = "0.15"
```

---

## 项目初始化

### 目录结构

```bash
mkdir -p src/searcher src/mcp
touch src/searcher/mod.rs src/mcp/mod.rs
```

### 基础文件配置

**src/main.rs (初始版本):**

```rust
use anyhow::Result;
use dotenv::dotenv;
use tracing::info;
use tracing_subscriber::fmt;

#[tokio::main]
async fn main() -> Result<()> {
    // 加载环境变量
    dotenv().ok();

    // 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting MCP server...");

    // TODO: 实现 MCP 服务器

    Ok(())
}
```

---

## 核心模块实现

### 1. 定义错误类型

**src/searcher/mod.rs:**

```rust
use thiserror::Error;

/// 自定义错误类型
#[derive(Error, Debug)]
pub enum SearcherError {
    #[error("HTTP request failed: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("JSON parsing failed: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Environment variable not set: {0}")]
    EnvVarNotSet(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
```

---

## 定义 API 客户端

### 2. 实现 Prometheus 客户端

**src/searcher/prometheus.rs:**

```rust
use super::SearcherError;
use serde::{Deserialize, Serialize};

// ========== 数据结构定义 ==========

/// 构建信息
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BuildInfo {
    pub version: String,
    pub revision: String,
    pub branch: String,
    #[serde(alias = "buildUser")]
    pub build_user: String,
    #[serde(alias = "buildDate")]
    pub build_date: String,
}

/// 查询响应
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResponse {
    pub data: QueryData,
    pub result_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum QueryData {
    Vector(Vec<Sample>),
    Matrix(Vec<RangeSample>),
    Scalar(ScalarSample),
    String(StringSample),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Sample {
    pub metric: std::collections::HashMap<String, String>,
    pub value: Vec<serde_json::Value>,
}

// ========== API 响应包装器 ==========

#[derive(Debug, Serialize, Deserialize)]
struct PrometheusResponse<T> {
    status: String,
    data: T,
}

// ========== 客户端实现 ==========

pub struct PrometheusClient {
    client: reqwest::Client,
    root: String,
}

impl PrometheusClient {
    pub fn new(root: String) -> Self {
        PrometheusClient {
            client: reqwest::Client::new(),
            root,
        }
    }

    /// 获取构建信息
    pub async fn build_info(&self) -> Result<BuildInfo, SearcherError> {
        let url = format!("{}/api/v1/status/buildinfo", self.root);
        let response = self.client.get(&url).send().await?;
        let prom_response: PrometheusResponse<BuildInfo> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    /// 执行 PromQL 查询
    pub async fn query(
        &self,
        query: &str,
        time: Option<&str>,
    ) -> Result<QueryResponse, SearcherError> {
        let mut request = self
            .client
            .get(format!("{}/api/v1/query", self.root))
            .query(&[("query", query)]);

        if let Some(t) = time {
            request = request.query(&[("time", t)]);
        }

        let response = request.send().await?;
        let prom_response: PrometheusResponse<QueryResponse> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    /// 范围查询
    pub async fn query_range(
        &self,
        query: &str,
        start: &str,
        end: &str,
        step: &str,
    ) -> Result<QueryRangeResponse, SearcherError> {
        let response = self
            .client
            .get(format!("{}/api/v1/query_range", self.root))
            .query(&[
                ("query", query),
                ("start", start),
                ("end", end),
                ("step", step),
            ])
            .send()
            .await?;

        let prom_response: PrometheusResponse<QueryRangeResponse> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }
}
```

### 3. 实现模块导出

**src/searcher/mod.rs:**

```rust
use std::env::var;

pub mod prometheus;
pub mod loki;

use prometheus::PrometheusClient;
use loki::LokiClient;

#[derive(Error, Debug)]
pub enum SearcherError {
    // ... 错误定义 ...
}

/// 从环境变量构建客户端
pub fn build_searcher() -> Result<Searcher, SearcherError> {
    let prometheus_root = var("PROMETHEUS_ROOT")
        .map_err(|_| SearcherError::EnvVarNotSet("PROMETHEUS_ROOT".to_string()))?;
    let loki_root = var("LOKI_ROOT")
        .map_err(|_| SearcherError::EnvVarNotSet("LOKI_ROOT".to_string()))?;

    Ok(Searcher {
        prometheus: PrometheusClient::new(prometheus_root),
        loki: LokiClient::new(loki_root),
    })
}

pub struct Searcher {
    pub prometheus: PrometheusClient,
    pub loki: LokiClient,
}
```

---

## 实现 MCP Tools

### 4. 定义 Tool 结构

**src/mcp/tools.rs:**

```rust
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo, ToolsCapability},
    schemars::JsonSchema,
    service::{NotificationContext, RequestContext},
    tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::searcher::Searcher;

// ========== 请求参数定义 ==========

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct QueryRequest {
    #[schemars(description = "PromQL 查询语句")]
    pub query: String,
    #[schemars(description = "可选的时间戳")]
    pub time: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct QueryRangeRequest {
    #[schemars(description = "PromQL 查询语句")]
    pub query: String,
    #[schemars(description = "开始时间戳")]
    pub start: String,
    #[schemars(description = "结束时间戳")]
    pub end: String,
    #[schemars(description = "查询步长")]
    pub step: String,
}

// ========== Tools 实现 ==========

pub struct Tools {
    tool_router: ToolRouter<Tools>,
    searcher: Searcher,
}

// 使用 proc macro 自动注册工具
#[tool_router(router = tool_router)]
impl Tools {
    pub fn new() -> Self {
        Tools {
            tool_router: Self::tool_router(),
            searcher: searcher::build_searcher()
                .expect("Failed to build searcher"),
        }
    }

    // ========== 简单工具 (无参数) ==========

    #[tool(description = "获取服务版本")]
    pub async fn version(&self) -> String {
        info!("Getting version");
        "1.0.0".to_string()
    }

    #[tool(description = "获取 Prometheus 构建信息")]
    pub async fn prom_build_info(&self) -> String {
        info!("Getting Prometheus build info");
        match self.searcher.prometheus.build_info().await {
            Ok(info) => serde_json::to_string(&info)
                .unwrap_or_else(|_| "Serialization failed".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    // ========== 带参数的工具 ==========

    #[tool(description = "执行 PromQL 即时查询")]
    pub async fn prom_query(
        &self,
        Parameters(params): Parameters<QueryRequest>,
    ) -> String {
        info!("Executing PromQL query: {}", params.query);
        match self.searcher
            .prometheus
            .query(&params.query, params.time.as_deref())
            .await
        {
            Ok(result) => serde_json::to_string(&result)
                .unwrap_or_else(|_| "Serialization failed".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "执行 PromQL 范围查询")]
    pub async fn prom_query_range(
        &self,
        Parameters(params): Parameters<QueryRangeRequest>,
    ) -> String {
        info!(
            "Query range: {} ({} to {}, step: {})",
            params.query, params.start, params.end, params.step
        );
        match self.searcher
            .prometheus
            .query_range(&params.query, &params.start, &params.end, &params.step)
            .await
        {
            Ok(result) => serde_json::to_string(&result)
                .unwrap_or_else(|_| "Serialization failed".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }
}

// ========== Server Handler 实现 ==========

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Tools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_06_18,
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                ..Default::default()
            },
            instructions: Some(
                "MCP Server for observability metrics".to_string(),
            ),
            server_info: Implementation {
                name: "my-mcp-server".to_string(),
                version: "0.1.0".to_string(),
                ..Default::default()
            },
        }
    }

    async fn ping(&self, _ctx: RequestContext<RoleServer>) -> Result<(), ErrorData> {
        info!("Received ping");
        Ok(())
    }

    async fn on_initialized(&self, _ctx: NotificationContext<RoleServer>) {
        info!("Client initialized");
    }
}
```

---

## 配置服务器

### 5. 实现主服务器

**src/main.rs:**

```rust
use anyhow::Result;
use dotenv::dotenv;
use rmcp::transport::{
    StreamableHttpServerConfig, StreamableHttpService,
    streamable_http_server::session::local::LocalSessionManager,
};
use tracing::info;
use tracing_subscriber::fmt;

mod docs;
mod mcp;
mod searcher;

use mcp::tools::Tools;

#[tokio::main]
async fn main() -> Result<()> {
    // 加载 .env 文件
    dotenv().ok();

    // 配置日志
    fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    info!("Starting MCP server...");

    // 创建取消令牌
    let ct = tokio_util::sync::CancellationToken::new();

    // 创建 MCP 服务
    let service = StreamableHttpService::new(
        || Ok(Tools::new()),
        LocalSessionManager::default().into(),
        StreamableHttpServerConfig {
            cancellation_token: ct.child_token(),
            ..Default::default()
        },
    );

    // 配置 Axum 路由
    let router = axum::Router::new()
        .nest_service("/mcp", service);

    // 绑定端口并启动服务
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3013").await?;
    info!("Server listening on http://0.0.0.0:3013/mcp");

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            tokio::signal::ctrl_c().await.unwrap();
            info!("Shutting down...");
            ct.cancel();
        })
        .await?;

    Ok(())
}
```

---

## 构建与运行

### 6. 配置环境变量

创建 `.env` 文件:

```bash
PROMETHEUS_ROOT=http://localhost:9090
LOKI_ROOT=http://localhost:3100
RUST_LOG=info
```

### 7. 构建项目

```bash
# 开发版本
cargo build

# 生产版本
cargo build --release
```

### 8. 运行服务器

```bash
# 开发模式
cargo run

# 生产模式
./target/release/my-mcp-server
```

服务器将在 `http://localhost:3013/mcp` 启动。

---

## 最佳实践

### 错误处理

使用 `thiserror` 定义清晰的错误类型:

```rust
#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("API returned error: {0}")]
    Api(String),

    #[error("Not found: {0}")]
    NotFound(String),
}
```

### 日志记录

使用结构化日志:

```rust
use tracing::{info, error, debug, instrument};

#[instrument(skip(self))]
pub async fn query(&self, query: &str) -> Result<Response> {
    info!("Executing query: {}", query);
    debug!("Query details: {:?}", query);

    match self.api_call(query).await {
        Ok(response) => {
            info!("Query successful");
            Ok(response)
        }
        Err(e) => {
            error!("Query failed: {}", e);
            Err(e)
        }
    }
}
```

### 参数验证

使用 `schemars` 添加详细描述:

```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct QueryRequest {
    #[schemars(description = "PromQL 查询语句")]
    #[schemars(example = "up")]
    pub query: String,

    #[schemars(description = "Unix 时间戳或 RFC3339 格式")]
    #[schemars(example = "2025-01-21T00:00:00Z")]
    pub time: Option<String>,
}
```

### 性能优化

1. **重用 HTTP 客户端**:
```rust
pub struct MyClient {
    client: reqwest::Client,  // 重用连接池
}
```

2. **使用异步并发**:
```rust
let (prometheus_data, loki_data) = tokio::try_join!(
    self.prometheus.query(query),
    self.loki.query(query)
)?;
```

3. **缓存结果** (适用时):
```rust
use tokio::sync::RwLock;
use std::time::Instant;

pub struct CachedClient {
    cache: RwLock<HashMap<String, (Instant, Response)>>,
}
```

### 安全建议

1. **敏感信息**:
```rust
// 不要在日志中输出敏感信息
info!("Query executed with params: {:?}", params);  // 避免
info!("Query executed");  // 推荐
```

2. **输入验证**:
```rust
pub async fn query(&self, query: &str) -> Result<Response> {
    if query.len() > 10000 {
        return Err(ApiError::InvalidInput("Query too long".into()));
    }
    // ...
}
```

3. **超时控制**:
```rust
use tokio::time::{timeout, Duration};

let response = timeout(
    Duration::from_secs(30),
    self.client.send(request)
).await??;
```

---

## 客户端集成

### Claude Desktop 配置

在 Claude Desktop 的配置文件中添加:

**macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
**Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "observability": {
      "url": "http://localhost:3013/mcp",
      "transport": "streamable-http"
    }
  }
}
```

### 测试工具

```bash
# 健康检查
curl http://localhost:3013/mcp

# 列出可用工具
curl -X POST http://localhost:3013/mcp/tools/list
```

---

## 调试技巧

### 启用详细日志

```bash
RUST_LOG=debug cargo run
```

### 使用 MCP Inspector

```bash
npx @modelcontextprotocol/inspect
```

### 单元测试示例

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_query() {
        let client = PrometheusClient::new("http://localhost:9090".into());
        let result = client.query("up", None).await;
        assert!(result.is_ok());
    }
}
```

---

## 参考资源

- [MCP 协议规范](https://modelcontextprotocol.io/)
- [rmcp 文档](https://github.com/juke6/rmcp)
- [Prometheus API 文档](https://prometheus.io/docs/prometheus/latest/querying/api/)
- [Axum 框架](https://github.com/tokio-rs/axum)

---

## 常见问题

**Q: 如何添加新的工具?**

A: 在 `Tools` impl 块中使用 `#[tool]` 宏添加方法，并定义相应的参数结构体。

**Q: 如何处理复杂的 JSON 响应?**

A: 使用 `serde` 的 `#[serde(untagged)]` 处理多态响应，使用 `alias` 处理命名不一致。

**Q: 如何支持 WebSocket 传输?**

A: 更换 `Cargo.toml` 中的 feature 为 `transport-ws-server`，并调整服务器配置。

---

**本指南持续更新中，欢迎贡献改进建议！**
