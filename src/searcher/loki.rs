use super::SearcherError;
use serde::{Deserialize, Serialize};

///
/// 放置 Loki 的操作相关的方法
///

/// Loki 标签
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Stream {
    #[serde(flatten)]
    pub labels: std::collections::HashMap<String, String>,
}

/// Loki 日志条目
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Entry {
    pub ts: String,
    pub line: String,
}

/// Loki 查询响应数据
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResponse {
    pub result_type: String,
    pub result: Vec<StreamResult>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum StreamResult {
    Stream {
        stream: std::collections::HashMap<String, String>,
        values: Vec<Vec<serde_json::Value>>,
    },
}

/// Loki 标签响应
#[derive(Debug, Serialize, Deserialize)]
pub struct LabelsResponse {
    pub data: Vec<String>,
}

/// Loki 标签值响应
#[derive(Debug, Serialize, Deserialize)]
pub struct LabelValuesResponse {
    pub data: Vec<String>,
}

/// Loki 统计信息响应
#[derive(Debug, Serialize, Deserialize)]
pub struct StatsResponse {
    pub stats: Stats,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Stats {
    pub ingestion_rate: Option<f64>,
    pub stream_rate_by_label: Option<std::collections::HashMap<String, f64>>,
}

/// Loki 构建信息
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BuildInfo {
    pub version: String,
    pub revision: String,
    pub branch: String,
    #[serde(alias = "buildUser")]
    pub build_user: String,
    #[serde(alias = "buildDate")]
    pub build_date: String,
    pub go_version: String,
}

/// Loki 配置响应
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub config: String,
}

/// Loki 限流响应
#[derive(Debug, Serialize, Deserialize)]
pub struct LimitsResponse {
    pub limits: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LokiResponse<T> {
    status: String,
    data: T,
}

pub struct LokiClient {
    client: reqwest::Client,
    root: String,
}

impl LokiClient {
    pub fn new(root: String) -> Self {
        LokiClient {
            client: reqwest::Client::new(),
            root: root,
        }
    }

    /// 查询日志
    ///
    /// # 参数
    /// - `query`: LogQL 查询语句
    /// - `start`: 开始时间 (RFC3339 格式或时间戳)
    /// - `end`: 结束时间 (RFC3339 格式或时间戳)
    /// - `limit`: 返回的最大条目数
    ///
    /// # 示例
    /// ```no_run
    /// let result = client.query("{job=\"myapp\"}", "2024-01-01T00:00:00Z", "2024-01-01T01:00:00Z", Some(100)).await?;
    /// ```
    pub async fn query(
        &self,
        query: &str,
        start: &str,
        end: &str,
        limit: Option<u32>,
    ) -> Result<QueryResponse, SearcherError> {
        let mut request = self
            .client
            .get(format!("{}/loki/api/v1/query_range", self.root))
            .query(&[
                ("query", query),
                ("start", start),
                ("end", end),
                ("limit", &limit.unwrap_or(100).to_string()),
            ]);

        let response = request.send().await?;
        let loki_response: LokiResponse<QueryResponse> = response.json().await?;

        if loki_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Loki API returned non-success status".to_string(),
            ));
        }

        Ok(loki_response.data)
    }

    /// 即时查询 (单次查询)
    pub async fn instant_query(
        &self,
        query: &str,
        time: Option<&str>,
        limit: Option<u32>,
    ) -> Result<QueryResponse, SearcherError> {
        let mut request = self
            .client
            .get(format!("{}/loki/api/v1/query", self.root))
            .query(&[
                ("query", query),
                ("limit", &limit.unwrap_or(100).to_string()),
            ]);

        if let Some(t) = time {
            request = request.query(&[("time", t)]);
        }

        let response = request.send().await?;
        let loki_response: LokiResponse<QueryResponse> = response.json().await?;

        if loki_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Loki API returned non-success status".to_string(),
            ));
        }

        Ok(loki_response.data)
    }

    /// 获取标签列表
    pub async fn labels(&self, start: Option<&str>, end: Option<&str>) -> Result<LabelsResponse, SearcherError> {
        let mut request = self
            .client
            .get(format!("{}/loki/api/v1/labels", self.root));

        if let Some(s) = start {
            request = request.query(&[("start", s)]);
        }
        if let Some(e) = end {
            request = request.query(&[("end", e)]);
        }

        let response = request.send().await?;
        let loki_response: LokiResponse<LabelsResponse> = response.json().await?;

        if loki_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Loki API returned non-success status".to_string(),
            ));
        }

        Ok(loki_response.data)
    }

    /// 获取标签值
    pub async fn label_values(
        &self,
        label_name: &str,
        start: Option<&str>,
        end: Option<&str>,
    ) -> Result<LabelValuesResponse, SearcherError> {
        let mut request = self
            .client
            .get(format!("{}/loki/api/v1/label/{}/values", self.root, label_name));

        if let Some(s) = start {
            request = request.query(&[("start", s)]);
        }
        if let Some(e) = end {
            request = request.query(&[("end", e)]);
        }

        let response = request.send().await?;
        let loki_response: LokiResponse<LabelValuesResponse> = response.json().await?;

        if loki_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Loki API returned non-success status".to_string(),
            ));
        }

        Ok(loki_response.data)
    }

    /// 获取构建信息
    pub async fn build_info(&self) -> Result<BuildInfo, SearcherError> {
        let url = format!("{}/loki/api/v1/status/buildinfo", self.root);
        let response = self.client.get(&url).send().await?;
        let loki_response: LokiResponse<BuildInfo> = response.json().await?;

        if loki_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Loki API returned non-success status".to_string(),
            ));
        }

        Ok(loki_response.data)
    }

    /// 获取配置
    pub async fn config(&self) -> Result<ConfigResponse, SearcherError> {
        let url = format!("{}/loki/api/v1/status/config", self.root);
        let response = self.client.get(&url).send().await?;
        let loki_response: LokiResponse<ConfigResponse> = response.json().await?;

        if loki_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Loki API returned non-success status".to_string(),
            ));
        }

        Ok(loki_response.data)
    }

    /// 获取限流配置
    pub async fn limits(&self) -> Result<LimitsResponse, SearcherError> {
        let url = format!("{}/loki/api/v1/limits", self.root);
        let response = self.client.get(&url).send().await?;
        let loki_response: LokiResponse<LimitsResponse> = response.json().await?;

        if loki_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Loki API returned non-success status".to_string(),
            ));
        }

        Ok(loki_response.data)
    }

    /// 获取统计信息
    pub async fn stats(&self) -> Result<StatsResponse, SearcherError> {
        let url = format!("{}/loki/api/v1/stats", self.root);
        let response = self.client.get(&url).send().await?;
        let loki_response: LokiResponse<StatsResponse> = response.json().await?;

        if loki_response.status != "success" {
            return Err(SearcherError::ApiError(
                "Loki API returned non-success status".to_string(),
            ));
        }

        Ok(loki_response.data)
    }

    /// 健康检查
    pub async fn healthy(&self) -> Result<String, SearcherError> {
        let url = format!("{}/ready", self.root);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(SearcherError::ApiError(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        Ok("Loki is healthy".to_string())
    }

    /// 就绪检查
    pub async fn ready(&self) -> Result<String, SearcherError> {
        let url = format!("{}/ready", self.root);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(SearcherError::ApiError(format!(
                "HTTP error: {}",
                response.status()
            )));
        }

        Ok("Loki is ready".to_string())
    }
}
