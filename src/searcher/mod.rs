use std::env::var;
use thiserror::Error;

pub mod prometheus;
pub mod loki;
pub mod harbor;
use prometheus::PrometheusClient;
use loki::LokiClient;
use harbor::HarborClient;

/// searcher 模块的错误类型
#[derive(Error, Debug)]
pub enum SearcherError {
    /// HTTP 请求错误
    #[error("HTTP request failed: {0}")]
    RequestError(#[from] reqwest::Error),

    /// JSON 反序列化错误
    #[error("Failed to parse JSON response: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Prometheus API 返回错误状态
    #[error("Prometheus API returned error status: {0}")]
    ApiError(String),

    /// 无效的 URL
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    /// IO 错误
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// 环境变量未设置
    #[error("Environment variable not set: {0}")]
    EnvVarNotSet(String),

    /// 其他错误
    #[error("Unknown error: {0}")]
    Other(String),
}

/// 从环境变量构建 Searcher 实例
///
/// # 环境变量
/// - `PROMETHEUS_ROOT`: Prometheus 服务器的根地址
/// - `LOKI_ROOT`: Loki 服务器的根地址
/// - `HARBOR_URL`: Harbor 服务器的 URL (可选)
/// - `HARBOR_USERNAME`: Harbor 用户名 (可选)
/// - `HARBOR_PASSWORD`: Harbor 密码 (可选)
///
/// # 示例
/// ```
/// let searcher = build_searcher().unwrap();
/// ```
pub fn build_searcher() -> Result<Searcher, SearcherError> {
    let prometheus_root = var("PROMETHEUS_ROOT")
        .map_err(|_| SearcherError::EnvVarNotSet("PROMETHEUS_ROOT".to_string()))?;
    let loki_root = var("LOKI_ROOT")
        .map_err(|_| SearcherError::EnvVarNotSet("LOKI_ROOT".to_string()))?;

    // Harbor 配置是可选的
    let harbor = if let (Ok(url), Ok(username), Ok(password)) = (
        var("HARBOR_URL"),
        var("HARBOR_USERNAME"),
        var("HARBOR_PASSWORD"),
    ) {
        Some(HarborClient::new(url, username, password))
    } else {
        None
    };

    Ok(Searcher {
        prometheus: PrometheusClient::new(prometheus_root),
        loki: LokiClient::new(loki_root),
        harbor,
    })
}

/// Searcher 结构体，包含 Prometheus、Loki 和 Harbor 客户端
pub struct Searcher {
    pub prometheus: PrometheusClient,
    pub loki: LokiClient,
    pub harbor: Option<HarborClient>,
}

impl Searcher {
    /// 使用指定的 Prometheus、Loki 和 Harbor 地址创建 Searcher 实例
    pub fn new(prometheus_root: String, loki_root: String, harbor: Option<HarborClient>) -> Self {
        Searcher {
            prometheus: PrometheusClient::new(prometheus_root),
            loki: LokiClient::new(loki_root),
            harbor,
        }
    }

    pub fn prometheus(&self) -> &PrometheusClient {
        &self.prometheus
    }

    pub fn loki(&self) -> &LokiClient {
        &self.loki
    }

    pub fn harbor(&self) -> Option<&HarborClient> {
        self.harbor.as_ref()
    }
}
