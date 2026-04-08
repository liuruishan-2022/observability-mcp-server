use std::env::var;
use thiserror::Error;

pub mod prometheus;
pub mod loki;
pub mod harbor;
pub mod nacos;
pub mod kubernetes;
pub mod kafka;
pub mod doris;
pub mod weixin;
use prometheus::PrometheusClient;
use loki::LokiClient;
use harbor::HarborClient;
use nacos::NacosClient;
use kubernetes::KubernetesClient;
use kafka::KafkaClient;
use doris::DorisClient;
use weixin::WeixinClient;

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
/// - `KAFKA_BOOTSTRAP_SERVERS`: Kafka bootstrap servers (可选)
/// - `KAFKA_CONSUMER_GROUP_ID`: Kafka consumer group ID (可选)
/// - `KAFKA_USERNAME`: Kafka SASL username (可选)
/// - `KAFKA_PASSWORD`: Kafka SASL password (可选)
/// - `KAFKA_SECURITY_PROTOCOL`: Kafka security protocol (可选)
/// - `DORIS_HOST`: Doris 主机地址 (可选)
/// - `DORIS_PORT`: Doris MySQL 协议端口 (可���, 默认 9030)
/// - `DORIS_USERNAME`: Doris 用户名 (可选)
/// - `DORIS_PASSWORD`: Doris 密码 (可选)
/// - `DORIS_DB`: Doris 数据库名 (可���)
/// - `DORIS_HTTP_URL`: Doris HTTP API URL (可选, e.g., http://host:8030)
/// - `WEIXIN_WEBHOOK_URL`: 企业微信机器人 Webhook URL (可选)
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

    // Kafka 配置是可选的
    let kafka = if let Ok(bootstrap_servers) = var("KAFKA_BOOTSTRAP_SERVERS") {
        let group_id = var("KAFKA_CONSUMER_GROUP_ID").ok();
        let username = var("KAFKA_USERNAME").ok();
        let password = var("KAFKA_PASSWORD").ok();
        let security_protocol = var("KAFKA_SECURITY_PROTOCOL").ok();
        Some(KafkaClient::new(bootstrap_servers, group_id, username, password, security_protocol)?)
    } else {
        None
    };

    // Doris 配置是可选的
    let doris = if let Ok(host) = var("DORIS_HOST") {
        let port = var("DORIS_PORT").unwrap_or_else(|_| "9030".to_string());
        if let (Ok(username), Ok(password), Ok(db)) = (
            var("DORIS_USERNAME"),
            var("DORIS_PASSWORD"),
            var("DORIS_DB"),
        ) {
            let http_url = var("DORIS_HTTP_URL").ok();
            Some(DorisClient::new(host, port, username, password, db, http_url))
        } else {
            None
        }
    } else {
        None
    };

    // 企业微信配置是可选的
    let weixin = if let Ok(webhook_url) = var("WEIXIN_WEBHOOK_URL") {
        Some(WeixinClient::new(webhook_url))
    } else {
        None
    };

    Ok(Searcher {
        prometheus: PrometheusClient::new(prometheus_root),
        loki: LokiClient::new(loki_root),
        harbor,
        nacos,
        kafka,
        doris,
        weixin,
        kubernetes: None,  // Kubernetes client requires async initialization
    })
}

/// 从运行时创建 Searcher 实例（用于需要异步初始化的客户端）
pub async fn build_searcher_async() -> Result<Searcher, SearcherError> {
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

    // Kafka 配置是可选的
    let kafka = if let Ok(bootstrap_servers) = var("KAFKA_BOOTSTRAP_SERVERS") {
        let group_id = var("KAFKA_CONSUMER_GROUP_ID").ok();
        let username = var("KAFKA_USERNAME").ok();
        let password = var("KAFKA_PASSWORD").ok();
        let security_protocol = var("KAFKA_SECURITY_PROTOCOL").ok();
        Some(KafkaClient::new(bootstrap_servers, group_id, username, password, security_protocol)?)
    } else {
        None
    };

    // Doris 配置是可选的
    let doris = if let Ok(host) = var("DORIS_HOST") {
        let port = var("DORIS_PORT").unwrap_or_else(|_| "9030".to_string());
        if let (Ok(username), Ok(password), Ok(db)) = (
            var("DORIS_USERNAME"),
            var("DORIS_PASSWORD"),
            var("DORIS_DB"),
        ) {
            let http_url = var("DORIS_HTTP_URL").ok();
            Some(DorisClient::new(host, port, username, password, db, http_url))
        } else {
            None
        }
    } else {
        None
    };

    // 企业微信配置是可选的
    let weixin = if let Ok(webhook_url) = var("WEIXIN_WEBHOOK_URL") {
        Some(WeixinClient::new(webhook_url))
    } else {
        None
    };

    // Kubernetes 客户端需要异步初始化
    let kubernetes = KubernetesClient::new(None).await.ok();

    Ok(Searcher {
        prometheus: PrometheusClient::new(prometheus_root),
        loki: LokiClient::new(loki_root),
        harbor,
        nacos,
        kafka,
        doris,
        weixin,
        kubernetes,
    })
}

/// Searcher 结构体，包含 Prometheus、Loki、Harbor、Nacos、Kafka、Doris、企业微信 和 Kubernetes 客户端
pub struct Searcher {
    pub prometheus: PrometheusClient,
    pub loki: LokiClient,
    pub harbor: Option<HarborClient>,
    pub nacos: Option<NacosClient>,
    pub kafka: Option<KafkaClient>,
    pub doris: Option<DorisClient>,
    pub weixin: Option<WeixinClient>,
    pub kubernetes: Option<KubernetesClient>,
}

impl Searcher {
    /// 使用指定的 Prometheus、Loki、Harbor、Nacos、Kafka、Doris、企业微信和 Kubernetes 地址创建 Searcher 实例
    pub fn new(
        prometheus_root: String,
        loki_root: String,
        harbor: Option<HarborClient>,
        nacos: Option<NacosClient>,
        kafka: Option<KafkaClient>,
        doris: Option<DorisClient>,
        weixin: Option<WeixinClient>,
        kubernetes: Option<KubernetesClient>,
    ) -> Self {
        Searcher {
            prometheus: PrometheusClient::new(prometheus_root),
            loki: LokiClient::new(loki_root),
            harbor,
            nacos,
            kafka,
            doris,
            weixin,
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

    pub fn kafka(&self) -> Option<&KafkaClient> {
        self.kafka.as_ref()
    }

    pub fn doris(&self) -> Option<&DorisClient> {
        self.doris.as_ref()
    }

    pub fn weixin(&self) -> Option<&WeixinClient> {
        self.weixin.as_ref()
    }

    pub fn kubernetes(&self) -> Option<&KubernetesClient> {
        self.kubernetes.as_ref()
    }
}
