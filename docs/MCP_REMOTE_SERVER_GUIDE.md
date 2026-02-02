# 从零开始构建你的第一个 MCP Remote Server

> 让 Claude 飞起来：为 AI 添加超能力

## 写在前面

你是否想过让 Claude 能够实时查询 Prometheus 指标？或者在对话中直接操作 Kubernetes 集群？又或者让 AI 帮你管理 Harbor 镜像仓库？

这一切都可以通过 **MCP (Model Context Protocol)** 实现！

今天，我们就来聊聊如何用 Rust 编写一个高性能的 MCP Remote Server，为你的 AI 助手装上"千里眼"和"顺风耳"。

![MCP架构](https://img.shields.io/badge/MCP-Protocol-blue)
![Rust](https://img.shields.io/badge/Rust-1.83+-orange)
![License](https://img.shields.io/badge/License-MIT-green)

---

## 📚 目录

1. [什么是 MCP Remote Server](#什么是-mcp-remote-server)
2. [为什么选择 Rust](#为什么选择-rust)
3. [五步上手实战](#五步上手实战)
4. [核心代码解析](#核心代码解析)
5. [最佳实践与踩坑指南](#最佳实践与踩坑指南)
6. [接下来做什么](#接下来做什么)

---

## 什么是 MCP Remote Server

**MCP (Model Context Protocol)** 是 Anthropic 开发的一个开放协议，它就像是 AI 和外部世界之间的"翻译官"。

### 传统的 Local MCP Server

```
Claude Desktop <--stdio--> MCP Server <--HTTP--> 外部服务
```

每个 MCP Server 都是一个独立的进程，通过 stdio 与 Claude Desktop 通信。

### Remote MCP Server 的优势

```
Claude Desktop <--HTTP/SSE--> MCP Remote Server <--HTTP--> 外部服务
```

✅ **集中部署**：一台服务器运行多个 MCP Server
✅ **资源共享**：复用数据库连接、缓存等
✅ **易于维护**：统一管理，不需要在每个客户端都安装
✅ **团队协作**：团队成员共享同一个服务

---

## 为什么选择 Rust

你可能问："为什么不用 Python 或 Node.js？"

### 🚀 性能怪兽

```
Rust: 0.1ms 响应时间
Python: 10ms 响应时间
Node.js: 5ms 响应时间
```

### 🔒 内存安全

Rust 的所有权系统在编译期就捕获了大部分内存错误，对于长期运行的服务器来说，这意味着**零内存泄漏**。

### 📦 强大的生态

- **Tokio**：异步运行时，性能媲美 C++
- **Axum**：现代化的 Web 框架
- **Serde**：序列化/反序列化的瑞士军刀
- **rmcp**：MCP 协议的 Rust 实现

---

## 五步上手实战

### 第一步：创建项目

```bash
# 安装 Rust（如果还没有）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 创建新项目
cargo new my-mcp-server
cd my-mcp-server
```

### 第二步：配置依赖

编辑 `Cargo.toml`：

```toml
[package]
name = "my-mcp-server"
version = "0.1.0"
edition = "2021"

[dependencies]
# 异步运行时 - Rust 的"发动机"
tokio = { version = "1.48", features = ["full"] }

# 错误处理 - 让错误信息更清晰
anyhow = "1.0"
thiserror = "2.0"

# 序列化 - JSON 处理必备
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# HTTP 服务器
axum = { version = "0.8", features = ["macros"] }

# HTTP 客户端
reqwest = { version = "0.12", features = ["json"] }

# 日志框架
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# MCP 核心库 - 我们的主角
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

### 第三步：项目结构

```bash
mkdir -p src/searcher src/mcp
touch src/searcher/mod.rs src/mcp/mod.rs
```

最终的目录结构：

```
my-mcp-server/
├── src/
│   ├── main.rs              # 🚀 入口文件
│   ├── searcher/            # 🔍 API 客户端
│   │   ├── mod.rs
│   │   ├── prometheus.rs    # Prometheus 客户端
│   │   └── loki.rs          # Loki 客户端
│   └── mcp/
│       ├── mod.rs
│       └── tools.rs         # 🛠️ MCP 工具定义
├── Cargo.toml
└── .env                     # 环境变量配置
```

### 第四步：编写核心代码

#### 1️⃣ 定义错误类型

**src/searcher/mod.rs**

```rust
use thiserror::Error;

/// 自定义错误类型 - 让错误处理更优雅
#[derive(Error, Debug)]
pub enum SearcherError {
    #[error("HTTP 请求失败: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("JSON 解析失败: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("API 错误: {0}")]
    ApiError(String),

    #[error("环境变量未设置: {0}")]
    EnvVarNotSet(String),

    #[error("IO 错误: {0}")]
    IoError(#[from] std::io::Error),
}
```

#### 2️⃣ 实现 Prometheus 客户端

**src/searcher/prometheus.rs**

```rust
use super::SearcherError;
use serde::{Deserialize, Serialize};

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

/// Prometheus 客户端
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

        #[derive(Deserialize)]
        struct PrometheusResponse<T> {
            status: String,
            data: T,
        }

        let prom_response: PrometheusResponse<BuildInfo> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }
}
```

#### 3️⃣ 实现 MCP 工具

**src/mcp/tools.rs**

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

/// 查询请求参数
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct QueryRequest {
    #[schemars(description = "PromQL 查询语句")]
    pub query: String,
    #[schemars(description = "可选的时间戳")]
    pub time: Option<String>,
}

/// MCP 工具集合
pub struct Tools {
    tool_router: ToolRouter<Tools>,
    searcher: Searcher,
}

// Rust 的魔法宏 - 自动注册工具
#[tool_router(router = tool_router)]
impl Tools {
    pub fn new() -> Self {
        Tools {
            tool_router: Self::tool_router(),
            searcher: searcher::build_searcher()
                .expect("Failed to build searcher"),
        }
    }

    /// 获取版本信息
    #[tool(description = "获取服务版本")]
    pub async fn version(&self) -> String {
        info!("Getting version");
        "1.0.0".to_string()
    }

    /// 获取 Prometheus 构建信息
    #[tool(description = "获取 Prometheus 构建信息")]
    pub async fn prom_build_info(&self) -> String {
        info!("Getting Prometheus build info");
        match self.searcher.prometheus.build_info().await {
            Ok(info) => serde_json::to_string(&info)
                .unwrap_or_else(|_| "Serialization failed".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    /// 执行 PromQL 查询
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
}

/// 实现 Server Handler
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
}
```

#### 4️⃣ 主服务器

**src/main.rs**

```rust
use anyhow::Result;
use dotenv::dotenv;
use rmcp::transport::{
    StreamableHttpServerConfig, StreamableHttpService,
    streamable_http_server::session::local::LocalSessionManager,
};
use tracing::info;
use tracing_subscriber::fmt;

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

    info!("🚀 Starting MCP server...");

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
    info!("✅ Server listening on http://0.0.0.0:3013/mcp");

    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            tokio::signal::ctrl_c().await.unwrap();
            info!("👋 Shutting down...");
            ct.cancel();
        })
        .await?;

    Ok(())
}
```

### 第五步：配置运行

**.env**

```bash
PROMETHEUS_ROOT=http://localhost:9090
LOKI_ROOT=http://localhost:3100
RUST_LOG=info
```

**构建并运行**

```bash
# 开发模式
cargo run

# 生产模式
cargo build --release
./target/release/my-mcp-server
```

---

## 核心代码解析

### 🎯 三个核心概念

#### 1. Tool（工具）

Tool 是 MCP 的核心，它定义了 AI 可以调用的功能。

```rust
#[tool(description = "执行 PromQL 即时查询")]
pub async fn prom_query(
    &self,
    Parameters(params): Parameters<QueryRequest>,
) -> String {
    // 实现逻辑
}
```

**关键点**：
- `#[tool]` 宏：自动注册为 MCP 工具
- `Parameters<T>`：自动解析 JSON 参数
- 返回 `String` 或 `Result<String, Error>`：返回给 AI 的结果

#### 2. ServerHandler（服务处理器）

定义 MCP Server 的基本信息和能力。

```rust
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
            // ...
        }
    }
}
```

#### 3. Transport（传输层）

Remote MCP Server 使用 HTTP + SSE（Server-Sent Events）。

```rust
let service = StreamableHttpService::new(
    || Ok(Tools::new()),
    LocalSessionManager::default().into(),
    StreamableHttpServerConfig {
        cancellation_token: ct.child_token(),
        ..Default::default()
    },
);
```

---

## 最佳实践与踩坑指南

### ✅ 推荐做法

#### 1. 错误处理

使用 `thiserror` 定义清晰的错误类型：

```rust
#[derive(Error, Debug)]
pub enum ApiError {
    #[error("网络错误: {0}")]
    Network(#[from] reqwest::Error),

    #[error("API 返回错误: {0}")]
    Api(String),
}
```

#### 2. 日志记录

使用结构化日志：

```rust
use tracing::{info, error, instrument};

#[instrument(skip(self))]
pub async fn query(&self, query: &str) -> Result<Response> {
    info!("执行查询: {}", query);

    match self.api_call(query).await {
        Ok(response) => {
            info!("查询成功");
            Ok(response)
        }
        Err(e) => {
            error!("查询失败: {}", e);
            Err(e)
        }
    }
}
```

#### 3. 参数验证

使用 `schemars` 添加详细描述：

```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct QueryRequest {
    #[schemars(description = "PromQL 查询语句")]
    #[schemars(example = "up")]
    pub query: String,

    #[schemars(description = "Unix 时间戳或 RFC3339 格式")]
    pub time: Option<String>,
}
```

### ❌ 常见错误

#### 错误 1：忘记实现 `ServerHandler`

```rust
// ❌ 缺少 ServerHandler 实现
pub struct Tools {
    // ...
}

// ✅ 添加 ServerHandler 实现
impl ServerHandler for Tools {
    fn get_info(&self) -> ServerInfo {
        // ...
    }
}
```

#### 错误 2：参数类型不匹配

```rust
// ❌ 错误的参数类型
pub async fn query(&self, params: QueryRequest) -> String {
    // ...
}

// ✅ 使用 Parameters 包装
pub async fn query(
    &self,
    Parameters(params): Parameters<QueryRequest>,
) -> String {
    // ...
}
```

#### 错误 3：环境变量未设置

```rust
// ❌ 直接使用环境变量
let root = std::env::var("PROMETHEUS_ROOT").unwrap();

// ✅ 优雅处理错误
let root = std::env::var("PROMETHEUS_ROOT")
    .map_err(|_| SearcherError::EnvVarNotSet("PROMETHEUS_ROOT".to_string()))?;
```

---

## 接下来做什么

### 🚀 扩展功能

1. **添加更多工具**：支持 Loki、Harbor、Kubernetes 等
2. **资源管理**：实现连接池、缓存机制
3. **认证鉴权**：添加 Token 验证、IP 白名单
4. **监控指标**：集成 Prometheus 暴露自身指标

### 📚 学习资源

- [MCP 协议规范](https://modelcontextprotocol.io/)
- [rmcp 文档](https://github.com/juke6/rmcp)
- [完整示例代码](https://github.com/yourusername/observability-mcp-server)

### 🤝 参与贡献

欢迎提交 Issue 和 Pull Request！

---

## 总结

通过这篇文章，我们学习了：

✅ 什么是 MCP Remote Server 及其优势
✅ 为什么选择 Rust 来实现
✅ 如何从零开始构建一个 MCP Server
✅ 核心概念和最佳实践

**Rust + MCP = 高性能 + 强扩展性**，为你的 AI 助力！

如果这篇文章对你有帮助，欢迎**点赞、收藏、转发**！

---

## 关注我们

- GitHub: [observability-mcp-server](https://github.com/yourusername/observability-mcp-server)
- 更多技术文章：[技术博客](https://yourblog.com)

**一键三连，我们下期再见！** 🎉
