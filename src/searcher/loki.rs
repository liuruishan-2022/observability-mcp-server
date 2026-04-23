use super::{SearcherError, global_http_ssl_verify, new_shared_http_client};
use serde::{Deserialize, Serialize};

///
/// 放置 Loki 的操作相关的方法
///

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
            client: new_shared_http_client(global_http_ssl_verify()),
            root: root,
        }
    }

    /// 查询日志
    ///
    /// # 参数
    /// - `query`: LogQL 查询语句
    /// - `start`: 开始时间 (RFC3339 格式或纳秒时间戳)
    /// - `end`: 结束时间 (RFC3339 格式或纳秒时间戳)
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
        let request = self
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

    /// 获取标签列表
    ///
    /// # 参数
    /// - `start`: 开始时间 (可选)
    /// - `end`: 结束时间 (可选)
    pub async fn labels(
        &self,
        start: Option<&str>,
        end: Option<&str>,
    ) -> Result<LabelsResponse, SearcherError> {
        let mut request = self.client.get(format!("{}/loki/api/v1/labels", self.root));

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
    ///
    /// # 参数
    /// - `label_name`: 标签名称
    /// - `start`: 开始时间 (可选)
    /// - `end`: 结束时间 (可选)
    pub async fn label_values(
        &self,
        label_name: &str,
        start: Option<&str>,
        end: Option<&str>,
    ) -> Result<LabelValuesResponse, SearcherError> {
        let mut request = self.client.get(format!(
            "{}/loki/api/v1/label/{}/values",
            self.root, label_name
        ));

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
}
