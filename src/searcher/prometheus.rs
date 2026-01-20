use super::SearcherError;
use serde::{Deserialize, Serialize};

///
/// 放置Prometheus的操作相关的方法
///

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AlertManager {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AlertManagersResponse {
    #[serde(alias = "activeAlertmanagers")]
    pub active_alert_managers: Vec<AlertManager>,
    #[serde(alias = "droppedAlertmanagers")]
    pub dropped_alert_managers: Vec<AlertManager>,
}

/// Prometheus 配置的 YAML 表示
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub yaml: String,
}

/// Prometheus 命令行标志
#[derive(Debug, Serialize, Deserialize)]
pub struct FlagsResponse {
    pub data: std::collections::HashMap<String, String>,
}

/// 运行时信息响应
#[derive(Debug, Serialize, Deserialize)]
pub struct RuntimeInfoResponse {
    pub data: RuntimeInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub start_time: String,
    pub uptime: String,
    pub cors: Option<bool>,
    pub chunk_encoding_version: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tsdb: Option<TsdbInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TsdbInfo {
    pub num_series: Option<u64>,
    pub num_label_pairs: Option<u64>,
    pub sample_count: Option<u64>,
    pub memory_in_bytes: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageInfo {
    pub name: String,
}

/// TSDB 状态响应
#[derive(Debug, Serialize, Deserialize)]
pub struct TsdbStatusResponse {
    pub data: TsdbStatusData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TsdbStatusData {
    pub head_stats: HeadStats,
    pub series_count_by_metric_name: Vec<std::collections::HashMap<String, u64>>,
    pub stats: TsdbStats,
    pub tombstones: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HeadStats {
    pub max_exemplar_ref: String,
    pub max_level: String,
    pub max_series_ref: String,
    pub num_exemplars: u64,
    pub num_label_pairs: u64,
    pub num_series: u64,
    pub num_series_refs: u64,
    pub num_tombstones: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TsdbStats {
    pub num_labels: u64,
    pub num_samples: u64,
    pub num_series: u64,
    pub series_count_by_metric_name: Vec<std::collections::HashMap<String, u64>>,
}

/// WAL Replay 状态响应
#[derive(Debug, Serialize, Deserialize)]
pub struct WalReplayResponse {
    pub data: WalReplayData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalReplayData {
    pub max_exemplar_ref: String,
    pub max_level: String,
    pub min_level: String,
    pub num_exemplars: u64,
    pub num_label_pairs: u64,
    pub num_series: u64,
    pub num_series_refs: u64,
    pub num_tombstones: u64,
}

/// 时序查询响应
#[derive(Debug, Serialize, Deserialize)]
pub struct SeriesResponse {
    pub data: Vec<SeriesData>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SeriesData {
    #[serde(flatten)]
    pub labels: std::collections::HashMap<String, String>,
}

/// 快照响应
#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotResponse {
    pub data: SnapshotData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotData {
    pub name: String,
}

/// Targets 元数据响应
#[derive(Debug, Serialize, Deserialize)]
pub struct TargetsMetadataResponse {
    pub data: Vec<TargetMetadata>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TargetMetadata {
    pub target: TargetInfo,
    pub metric: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TargetInfo {
    pub labels: std::collections::HashMap<String, String>,
}

/// Targets 响应
#[derive(Debug, Serialize, Deserialize)]
pub struct TargetsResponse {
    pub data: TargetsData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TargetsData {
    pub active_targets: Vec<Target>,
    pub dropped_targets: Vec<Target>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Target {
    pub labels: std::collections::HashMap<String, String>,
    pub scrape_pool: String,
    pub scrape_url: String,
    pub health: String,
    pub last_error: Option<String>,
}

/// 标签列表响应
#[derive(Debug, Serialize, Deserialize)]
pub struct LabelsResponse {
    pub data: Vec<String>,
}

/// 标签值响应
#[derive(Debug, Serialize, Deserialize)]
pub struct LabelValuesResponse {
    pub data: Vec<String>,
}

/// 告警响应
#[derive(Debug, Serialize, Deserialize)]
pub struct AlertsResponse {
    pub data: AlertsData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AlertsData {
    pub alerts: Vec<Alert>,
}

/// 规则响应
#[derive(Debug, Serialize, Deserialize)]
pub struct RulesResponse {
    pub data: RulesData,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RulesData {
    pub groups: Vec<RuleGroup>,
}

/// 指标元数据响应
#[derive(Debug, Serialize, Deserialize)]
pub struct MetadataResponse {
    pub data: std::collections::HashMap<String, Vec<MetricMetadata>>,
}

/// 查询响应
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResponse {
    pub data: QueryData,
    pub result_type: String,
}

/// 范围查询响应
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryRangeResponse {
    pub data: QueryRangeData,
    pub result_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryRangeData {
    pub result: Vec<RangeSample>,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScalarSample {
    pub value: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RangeSample {
    pub metric: std::collections::HashMap<String, String>,
    pub values: Vec<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StringSample {
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MetricMetadata {
    #[serde(rename = "type")]
    pub metric_type: String,
    pub help: String,
    pub unit: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RuleGroup {
    pub name: String,
    pub file: String,
    pub interval: String,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Rule {
    #[serde(rename = "alert")]
    AlertRule {
        name: String,
        query: String,
        duration: String,
        labels: std::collections::HashMap<String, String>,
        annotations: std::collections::HashMap<String, String>,
        alerts: Vec<Alert>,
        health: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        last_error: Option<String>,
    },
    #[serde(rename = "record")]
    RecordRule {
        name: String,
        query: String,
        labels: Option<std::collections::HashMap<String, String>>,
        health: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        last_error: Option<String>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Alert {
    pub fingerprint: String,
    pub labels: std::collections::HashMap<String, String>,
    pub annotations: std::collections::HashMap<String, String>,
    pub starts_at: String,
    pub ends_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Exemplar {
    pub labels: std::collections::HashMap<String, String>,
    pub value: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExemplarsResponse {
    pub result: Vec<Exemplar>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PrometheusResponse<T> {
    status: String,
    data: T,
}

pub struct PrometheusClient {
    client: reqwest::Client,
    root: String,
}

impl PrometheusClient {
    pub fn new(root: String) -> Self {
        PrometheusClient {
            client: reqwest::Client::new(),
            root: root,
        }
    }

    pub async fn build_info(&self) -> Result<BuildInfo, SearcherError> {
        let url = format!("{}/api/v1/status/buildinfo", self.root);
        let response = self.client.get(&url).send().await?;
        let prom_response: PrometheusResponse<BuildInfo> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    pub async fn alert_managers(&self) -> Result<AlertManagersResponse, SearcherError> {
        let url = format!("{}/api/v1/alertmanagers", self.root);
        let response = self.client.get(&url).send().await?;
        let prom_response: PrometheusResponse<AlertManagersResponse> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    pub async fn clean_tombstones(&self) -> Result<String, SearcherError> {
        let url = format!("{}/api/v1/admin/tsdb/clean_tombstones", self.root);
        let response = self.client.post(&url).send().await?;

        if !response.status().is_success() {
            return Err(SearcherError::ApiError(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        Ok("Tombstones cleaned successfully".to_string())
    }

    pub async fn delete_series(&self, matches: &[String]) -> Result<String, SearcherError> {
        let url = format!("{}/api/v1/admin/tsdb/delete_series", self.root);
        let mut request = self.client.post(&url);

        for match_str in matches {
            request = request.query(&[("match[]", match_str)]);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            return Err(SearcherError::ApiError(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        Ok("Series deleted successfully".to_string())
    }

    pub async fn config(&self) -> Result<ConfigResponse, SearcherError> {
        let url = format!("{}/api/v1/status/config", self.root);
        let response = self.client.get(&url).send().await?;
        let prom_response: PrometheusResponse<ConfigResponse> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    pub async fn query_exemplars(&self, query: &str) -> Result<ExemplarsResponse, SearcherError> {
        let url = format!("{}/api/v1/query_exemplars", self.root);
        let response = self
            .client
            .get(&url)
            .query(&[("query", query)])
            .send()
            .await?;
        let prom_response: PrometheusResponse<ExemplarsResponse> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    pub async fn flags(&self) -> Result<FlagsResponse, SearcherError> {
        let url = format!("{}/api/v1/status/flags", self.root);
        let response = self.client.get(&url).send().await?;
        let prom_response: PrometheusResponse<FlagsResponse> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    pub async fn labels(&self) -> Result<LabelsResponse, SearcherError> {
        let url = format!("{}/api/v1/labels", self.root);
        let response = self.client.get(&url).send().await?;
        let prom_response: PrometheusResponse<LabelsResponse> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    pub async fn label_values(
        &self,
        label_name: &str,
    ) -> Result<LabelValuesResponse, SearcherError> {
        let url = format!("{}/api/v1/label/{}/values", self.root, label_name);
        let response = self.client.get(&url).send().await?;
        let prom_response: PrometheusResponse<LabelValuesResponse> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    pub async fn alerts(&self) -> Result<AlertsResponse, SearcherError> {
        let url = format!("{}/api/v1/alerts", self.root);
        let response = self.client.get(&url).send().await?;
        let prom_response: PrometheusResponse<AlertsResponse> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    pub async fn rules(&self) -> Result<RulesResponse, SearcherError> {
        let url = format!("{}/api/v1/rules", self.root);
        let response = self.client.get(&url).send().await?;
        let prom_response: PrometheusResponse<RulesResponse> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    pub async fn metadata(&self) -> Result<MetadataResponse, SearcherError> {
        let url = format!("{}/api/v1/metadata", self.root);
        let response = self.client.get(&url).send().await?;
        let prom_response: PrometheusResponse<MetadataResponse> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

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
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

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
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    pub async fn runtime_info(&self) -> Result<RuntimeInfoResponse, SearcherError> {
        let url = format!("{}/api/v1/status/runtimeinfo", self.root);
        let response = self.client.get(&url).send().await?;
        let prom_response: PrometheusResponse<RuntimeInfoResponse> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    pub async fn series(&self, matches: &[String]) -> Result<SeriesResponse, SearcherError> {
        let mut request = self.client.get(format!("{}/api/v1/series", self.root));
        for match_str in matches {
            request = request.query(&[("match[]", match_str)]);
        }

        let response = request.send().await?;
        let prom_response: PrometheusResponse<SeriesResponse> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    pub async fn snapshot(&self) -> Result<SnapshotResponse, SearcherError> {
        let url = format!("{}/api/v1/admin/tsdb/snapshot", self.root);
        let response = self.client.post(&url).send().await?;
        let prom_response: PrometheusResponse<SnapshotResponse> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    pub async fn targets_metadata(
        &self,
        metric: Option<&str>,
        label: Option<&str>,
    ) -> Result<TargetsMetadataResponse, SearcherError> {
        let mut request = self
            .client
            .get(format!("{}/api/v1/targets/metadata", self.root));

        if let Some(m) = metric {
            request = request.query(&[("metric", m)]);
        }
        if let Some(l) = label {
            request = request.query(&[("label", l)]);
        }

        let response = request.send().await?;
        let prom_response: PrometheusResponse<TargetsMetadataResponse> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    pub async fn targets(&self) -> Result<TargetsResponse, SearcherError> {
        let url = format!("{}/api/v1/targets", self.root);
        let response = self.client.get(&url).send().await?;
        let prom_response: PrometheusResponse<TargetsResponse> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    pub async fn tsdb_status(&self) -> Result<TsdbStatusResponse, SearcherError> {
        let url = format!("{}/api/v1/status/tsdb", self.root);
        let response = self.client.get(&url).send().await?;
        let prom_response: PrometheusResponse<TsdbStatusResponse> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    pub async fn wal_replay(&self) -> Result<WalReplayResponse, SearcherError> {
        let url = format!("{}/api/v1/status/walreplay", self.root);
        let response = self.client.get(&url).send().await?;
        let prom_response: PrometheusResponse<WalReplayResponse> = response.json().await?;

        if prom_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Prometheus API returned non-success status".to_string(),
            ));
        }

        Ok(prom_response.data)
    }

    /// 健康检查
    pub async fn healthy(&self) -> Result<String, SearcherError> {
        let url = format!("{}/-/healthy", self.root);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(SearcherError::ApiError(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        Ok("Prometheus is healthy".to_string())
    }

    /// 就绪检查
    pub async fn ready(&self) -> Result<String, SearcherError> {
        let url = format!("{}/-/ready", self.root);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(SearcherError::ApiError(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        Ok("Prometheus is ready".to_string())
    }

    /// 重载配置
    pub async fn reload(&self) -> Result<String, SearcherError> {
        let url = format!("{}/-/reload", self.root);
        let response = self.client.post(&url).send().await?;

        if !response.status().is_success() {
            return Err(SearcherError::ApiError(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        Ok("Configuration reload triggered".to_string())
    }

    /// 退出服务
    pub async fn quit(&self) -> Result<String, SearcherError> {
        let url = format!("{}/-/quit", self.root);
        let response = self.client.post(&url).send().await?;

        if !response.status().is_success() {
            return Err(SearcherError::ApiError(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        Ok("Shutdown initiated".to_string())
    }
}
