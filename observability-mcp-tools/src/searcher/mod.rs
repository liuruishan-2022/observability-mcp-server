use reqwest::{Client, ClientBuilder, header::HeaderMap};
use std::env::var;
use std::error::Error as StdError;
use std::time::Duration;
use thiserror::Error;

pub mod atlassian;
pub mod doris;
pub mod grafana;
pub mod harbor;
pub mod kafka;
pub mod kubernetes;
pub mod loki;
pub mod nacos;
pub mod prometheus;
pub mod weixin;
use atlassian::{BitbucketClient, ConfluenceClient, JiraClient};
use doris::DorisClient;
use grafana::GrafanaClient;
use harbor::HarborClient;
use kafka::KafkaClient;
use kubernetes::KubernetesClient;
use loki::LokiClient;
use nacos::NacosClient;
use prometheus::PrometheusClient;
use weixin::WeixinClient;

/// searcher 模块的错误类型
#[derive(Error, Debug)]
pub enum SearcherError {
    /// HTTP 请求错误
    #[error("HTTP request failed: {}", format_reqwest_error(.0))]
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

const DEFAULT_HTTP_SSL_VERIFY: bool = false;
const DEFAULT_HTTP_CONNECT_TIMEOUT_SECS: u64 = 10;
const DEFAULT_HTTP_TIMEOUT_SECS: u64 = 30;

fn format_reqwest_error(error: &reqwest::Error) -> String {
    let mut details = vec![error.to_string()];

    if let Some(url) = error.url() {
        details.push(format!("url={url}"));
    }
    if let Some(status) = error.status() {
        details.push(format!("status={status}"));
    }
    if error.is_timeout() {
        details.push("kind=timeout".to_string());
    }
    if error.is_connect() {
        details.push("kind=connect".to_string());
    }
    if error.is_request() {
        details.push("kind=request".to_string());
    }
    if error.is_body() {
        details.push("kind=body".to_string());
    }
    if error.is_decode() {
        details.push("kind=decode".to_string());
    }

    let mut sources = Vec::new();
    let mut source = StdError::source(error);
    while let Some(current) = source {
        let message = current.to_string();
        if !message.is_empty() {
            sources.push(message);
        }
        source = current.source();
    }

    if !sources.is_empty() {
        details.push(format!("causes={}", sources.join(" -> ")));
    }

    details.join(" | ")
}

pub(crate) fn global_http_ssl_verify() -> bool {
    env_bool("HTTP_SSL_VERIFY", DEFAULT_HTTP_SSL_VERIFY)
}

fn shared_http_client_builder(ssl_verify: bool) -> ClientBuilder {
    Client::builder()
        .danger_accept_invalid_certs(!ssl_verify)
        .connect_timeout(Duration::from_secs(DEFAULT_HTTP_CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS))
}

pub(crate) fn build_shared_http_client(ssl_verify: bool) -> Result<Client, SearcherError> {
    shared_http_client_builder(ssl_verify)
        .build()
        .map_err(SearcherError::RequestError)
}

pub(crate) fn build_shared_http_client_with_headers(
    ssl_verify: bool,
    headers: HeaderMap,
) -> Result<Client, SearcherError> {
    shared_http_client_builder(ssl_verify)
        .default_headers(headers)
        .build()
        .map_err(SearcherError::RequestError)
}

pub(crate) fn new_shared_http_client(ssl_verify: bool) -> Client {
    build_shared_http_client(ssl_verify).expect("Failed to create HTTP client")
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
/// - `HTTP_SSL_VERIFY`: 全局 reqwest TLS 证书校验开关，默认 false
/// - `WEIXIN_WEBHOOK_URL`: 企业微信机器人 Webhook URL (可选)
/// - `JIRA_URL`: Jira 基础 URL (可选)
/// - `JIRA_USERNAME` / `JIRA_API_TOKEN`: Jira Basic 认证 (可选)
/// - `JIRA_PERSONAL_TOKEN`: Jira PAT 认证 (可选)
/// - `JIRA_PROJECTS_FILTER`: Jira 项目过滤器 (可选)
/// - `CONFLUENCE_URL`: Confluence 基础 URL (可选)
/// - `CONFLUENCE_USERNAME` / `CONFLUENCE_API_TOKEN`: Confluence Basic 认证 (可选)
/// - `CONFLUENCE_PERSONAL_TOKEN`: Confluence PAT 认证 (可选)
/// - `CONFLUENCE_SPACES_FILTER`: Confluence 空间过滤器 (可选)
/// - `BITBUCKET_URL`: Bitbucket Server/Data Center 基础 URL (可选)
/// - `BITBUCKET_USERNAME` / `BITBUCKET_PASSWORD`: Bitbucket Basic 认证 (可选)
/// - `BITBUCKET_PERSONAL_TOKEN`: Bitbucket PAT 认证 (可选)
/// - `BITBUCKET_PROJECT`: 默认 Bitbucket project key (可选)
/// - `GRAFANA_URL`: Grafana 基础 URL (可选)
/// - `GRAFANA_SERVICE_ACCOUNT_TOKEN`: Grafana service account token (可选)
/// - `GRAFANA_USERNAME` / `GRAFANA_PASSWORD`: Grafana Basic 认证 (可选)
///
/// # 示例
/// ```
/// let searcher = build_searcher().unwrap();
/// ```
pub fn build_searcher() -> Result<Searcher, SearcherError> {
    let prometheus_root = var("PROMETHEUS_ROOT")
        .map_err(|_| SearcherError::EnvVarNotSet("PROMETHEUS_ROOT".to_string()))?;
    let loki_root =
        var("LOKI_ROOT").map_err(|_| SearcherError::EnvVarNotSet("LOKI_ROOT".to_string()))?;

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
        Some(KafkaClient::new(
            bootstrap_servers,
            group_id,
            username,
            password,
            security_protocol,
        )?)
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
            Some(DorisClient::new(
                host, port, username, password, db, http_url,
            ))
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

    // Jira 配置是可选的
    let jira = if let Ok(url) = var("JIRA_URL") {
        let username = var("JIRA_USERNAME").ok();
        let api_token = var("JIRA_API_TOKEN").ok();
        let personal_token = var("JIRA_PERSONAL_TOKEN").ok();
        let ssl_verify = global_http_ssl_verify();
        let projects_filter = var("JIRA_PROJECTS_FILTER").ok();

        if personal_token.is_some() || (username.is_some() && api_token.is_some()) {
            Some(JiraClient::new(
                url,
                username,
                api_token,
                personal_token,
                ssl_verify,
                projects_filter,
            )?)
        } else {
            None
        }
    } else {
        None
    };

    // Confluence 配置是可选的
    let confluence = if let Ok(url) = var("CONFLUENCE_URL") {
        let username = var("CONFLUENCE_USERNAME").ok();
        let api_token = var("CONFLUENCE_API_TOKEN").ok();
        let personal_token = var("CONFLUENCE_PERSONAL_TOKEN").ok();
        let ssl_verify = global_http_ssl_verify();
        let spaces_filter = var("CONFLUENCE_SPACES_FILTER").ok();

        if personal_token.is_some() || (username.is_some() && api_token.is_some()) {
            Some(ConfluenceClient::new(
                url,
                username,
                api_token,
                personal_token,
                ssl_verify,
                spaces_filter,
            )?)
        } else {
            None
        }
    } else {
        None
    };

    // Bitbucket Server/Data Center 配置是可选的
    let bitbucket = if let Ok(url) = var("BITBUCKET_URL") {
        let username = var("BITBUCKET_USERNAME").ok();
        let password = var("BITBUCKET_PASSWORD").ok();
        let personal_token = var("BITBUCKET_PERSONAL_TOKEN")
            .ok()
            .or_else(|| var("BITBUCKET_TOKEN").ok());
        let ssl_verify = global_http_ssl_verify();
        let default_project = var("BITBUCKET_PROJECT").ok();

        if personal_token.is_some() || (username.is_some() && password.is_some()) {
            Some(BitbucketClient::new(
                url,
                username,
                password,
                personal_token,
                ssl_verify,
                default_project,
            )?)
        } else {
            None
        }
    } else {
        None
    };

    let grafana = if let Ok(url) = var("GRAFANA_URL") {
        let token = var("GRAFANA_SERVICE_ACCOUNT_TOKEN")
            .ok()
            .or_else(|| var("GRAFANA_TOKEN").ok());
        let username = var("GRAFANA_USERNAME").ok();
        let password = var("GRAFANA_PASSWORD").ok();
        if token.is_some() || (username.is_some() && password.is_some()) {
            Some(GrafanaClient::new(url, token, username, password)?)
        } else {
            None
        }
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
        jira,
        confluence,
        bitbucket,
        grafana,
        kubernetes: None, // Kubernetes client requires async initialization
    })
}

/// 从运行时创建 Searcher 实例（用于需要异步初始化的客户端）
pub async fn build_searcher_async() -> Result<Searcher, SearcherError> {
    let prometheus_root = var("PROMETHEUS_ROOT")
        .map_err(|_| SearcherError::EnvVarNotSet("PROMETHEUS_ROOT".to_string()))?;
    let loki_root =
        var("LOKI_ROOT").map_err(|_| SearcherError::EnvVarNotSet("LOKI_ROOT".to_string()))?;

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
        Some(KafkaClient::new(
            bootstrap_servers,
            group_id,
            username,
            password,
            security_protocol,
        )?)
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
            Some(DorisClient::new(
                host, port, username, password, db, http_url,
            ))
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

    // Jira 配置是可选的
    let jira = if let Ok(url) = var("JIRA_URL") {
        let username = var("JIRA_USERNAME").ok();
        let api_token = var("JIRA_API_TOKEN").ok();
        let personal_token = var("JIRA_PERSONAL_TOKEN").ok();
        let ssl_verify = global_http_ssl_verify();
        let projects_filter = var("JIRA_PROJECTS_FILTER").ok();

        if personal_token.is_some() || (username.is_some() && api_token.is_some()) {
            Some(JiraClient::new(
                url,
                username,
                api_token,
                personal_token,
                ssl_verify,
                projects_filter,
            )?)
        } else {
            None
        }
    } else {
        None
    };

    // Confluence 配置是可选的
    let confluence = if let Ok(url) = var("CONFLUENCE_URL") {
        let username = var("CONFLUENCE_USERNAME").ok();
        let api_token = var("CONFLUENCE_API_TOKEN").ok();
        let personal_token = var("CONFLUENCE_PERSONAL_TOKEN").ok();
        let ssl_verify = global_http_ssl_verify();
        let spaces_filter = var("CONFLUENCE_SPACES_FILTER").ok();

        if personal_token.is_some() || (username.is_some() && api_token.is_some()) {
            Some(ConfluenceClient::new(
                url,
                username,
                api_token,
                personal_token,
                ssl_verify,
                spaces_filter,
            )?)
        } else {
            None
        }
    } else {
        None
    };

    // Bitbucket Server/Data Center 配置是可选的
    let bitbucket = if let Ok(url) = var("BITBUCKET_URL") {
        let username = var("BITBUCKET_USERNAME").ok();
        let password = var("BITBUCKET_PASSWORD").ok();
        let personal_token = var("BITBUCKET_PERSONAL_TOKEN")
            .ok()
            .or_else(|| var("BITBUCKET_TOKEN").ok());
        let ssl_verify = global_http_ssl_verify();
        let default_project = var("BITBUCKET_PROJECT").ok();

        if personal_token.is_some() || (username.is_some() && password.is_some()) {
            Some(BitbucketClient::new(
                url,
                username,
                password,
                personal_token,
                ssl_verify,
                default_project,
            )?)
        } else {
            None
        }
    } else {
        None
    };

    let grafana = if let Ok(url) = var("GRAFANA_URL") {
        let token = var("GRAFANA_SERVICE_ACCOUNT_TOKEN")
            .ok()
            .or_else(|| var("GRAFANA_TOKEN").ok());
        let username = var("GRAFANA_USERNAME").ok();
        let password = var("GRAFANA_PASSWORD").ok();
        if token.is_some() || (username.is_some() && password.is_some()) {
            Some(GrafanaClient::new(url, token, username, password)?)
        } else {
            None
        }
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
        jira,
        confluence,
        bitbucket,
        grafana,
        kubernetes,
    })
}

/// Searcher 结构体，包含 Prometheus、Loki、Harbor、Nacos、Kafka、Doris、企业微信、Jira、Confluence、Bitbucket 和 Kubernetes 客户端
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
    pub grafana: Option<GrafanaClient>,
    pub kubernetes: Option<KubernetesClient>,
}

impl Searcher {
    /// 使用指定的 Prometheus、Loki、Harbor、Nacos、Kafka、Doris、企业微信、Jira、Confluence 和 Kubernetes 地址创建 Searcher 实例
    pub fn new(
        prometheus_root: String,
        loki_root: String,
        harbor: Option<HarborClient>,
        nacos: Option<NacosClient>,
        kafka: Option<KafkaClient>,
        doris: Option<DorisClient>,
        weixin: Option<WeixinClient>,
        jira: Option<JiraClient>,
        confluence: Option<ConfluenceClient>,
        bitbucket: Option<BitbucketClient>,
        grafana: Option<GrafanaClient>,
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
            jira,
            confluence,
            bitbucket,
            grafana,
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

    pub fn jira(&self) -> Option<&JiraClient> {
        self.jira.as_ref()
    }

    pub fn confluence(&self) -> Option<&ConfluenceClient> {
        self.confluence.as_ref()
    }

    pub fn bitbucket(&self) -> Option<&BitbucketClient> {
        self.bitbucket.as_ref()
    }

    pub fn grafana(&self) -> Option<&GrafanaClient> {
        self.grafana.as_ref()
    }

    pub fn kubernetes(&self) -> Option<&KubernetesClient> {
        self.kubernetes.as_ref()
    }
}

fn env_bool(name: &str, default: bool) -> bool {
    match var(name) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "false" | "0" | "no" | "off" => false,
            "true" | "1" | "yes" | "on" => true,
            _ => default,
        },
        Err(_) => default,
    }
}
