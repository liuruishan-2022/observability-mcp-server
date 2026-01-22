use std::env::var;
use thiserror::Error;

pub mod prometheus;
pub mod loki;
pub mod harbor;
pub mod nacos;
pub mod kubernetes;
use prometheus::PrometheusClient;
use loki::LokiClient;
use harbor::HarborClient;
use nacos::NacosClient;
use kubernetes::KubernetesClient;

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
/// - `NACOS_URL`: Nacos 服务器的 URL (可选)
/// - `NACOS_ACCESS_TOKEN`: Nacos 访问令牌 (可选)
/// - `KUBERNETES_CONTEXT`: Kubernetes context 名称 (可选)
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

    // Nacos 配置是可选的
    let nacos = if let Ok(url) = var("NACOS_URL") {
        let token = var("NACOS_ACCESS_TOKEN").ok();
        Some(NacosClient::new(url, token))
    } else {
        None
    };

    // Kubernetes context 是可选的
    let k8s_context = var("KUBERNETES_CONTEXT").ok();

    Ok(Searcher {
        prometheus: PrometheusClient::new(prometheus_root),
        loki: LokiClient::new(loki_root),
        harbor,
        nacos,
        k8s_context,
        kubernetes: None,  // Will be initialized later
    })
}

/// Searcher 结构体，包含 Prometheus、Loki、Harbor、Nacos 和 Kubernetes 客户端
pub struct Searcher {
    pub prometheus: PrometheusClient,
    pub loki: LokiClient,
    pub harbor: Option<HarborClient>,
    pub nacos: Option<NacosClient>,
    pub k8s_context: Option<String>,
    pub kubernetes: Option<KubernetesClient>,
}

impl Searcher {
    /// 使用指定的 Prometheus、Loki、Harbor、Nacos 和 Kubernetes 地址创建 Searcher 实例
    pub fn new(
        prometheus_root: String,
        loki_root: String,
        harbor: Option<HarborClient>,
        nacos: Option<NacosClient>,
        k8s_context: Option<String>,
        kubernetes: Option<KubernetesClient>,
    ) -> Self {
        Searcher {
            prometheus: PrometheusClient::new(prometheus_root),
            loki: LokiClient::new(loki_root),
            harbor,
            nacos,
            k8s_context,
            kubernetes,
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

    pub fn nacos(&self) -> Option<&NacosClient> {
        self.nacos.as_ref()
    }

    pub fn kubernetes(&self) -> Option<&KubernetesClient> {
        self.kubernetes.as_ref()
    }

    /// 初始化 Kubernetes 客户端
    pub async fn init_kubernetes(&mut self) -> Result<(), SearcherError> {
        if self.kubernetes.is_none() {
            let client = KubernetesClient::new(self.k8s_context.clone()).await?;
            self.kubernetes = Some(client);
        }
        Ok(())
    }
}
