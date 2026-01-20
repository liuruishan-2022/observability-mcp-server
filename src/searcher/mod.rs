use std::env::var;
use thiserror::Error;

pub mod prometheus;
use prometheus::PrometheusClient;

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
///
/// # 示例
/// ```
/// let searcher = build_searcher().unwrap();
/// ```
pub fn build_searcher() -> Result<Searcher, SearcherError> {
    let root = var("PROMETHEUS_ROOT")
        .map_err(|_| SearcherError::EnvVarNotSet("PROMETHEUS_ROOT".to_string()))?;

    Ok(Searcher {
        prometheus: PrometheusClient::new(root),
    })
}

/// Searcher 结构体，包含 Prometheus 客户端
pub struct Searcher {
    pub prometheus: PrometheusClient,
}

impl Searcher {
    /// 使用指定的 Prometheus 地址创建 Searcher 实例
    pub fn new(root: String) -> Self {
        Searcher {
            prometheus: PrometheusClient::new(root),
        }
    }

    pub fn prometheus(&self) -> &PrometheusClient {
        &self.prometheus
    }
}
