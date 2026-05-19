use crate::searcher::{
    SearcherError, build_shared_http_client_with_headers, global_http_ssl_verify,
};
use reqwest::{Client, Method, header};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone)]
pub struct GrafanaClient {
    client: Client,
    base_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GrafanaSearchResult {
    pub id: Option<i64>,
    pub uid: Option<String>,
    pub title: Option<String>,
    pub uri: Option<String>,
    pub url: Option<String>,
    #[serde(rename = "type")]
    pub item_type: Option<String>,
    pub tags: Option<Vec<String>>,
    #[serde(rename = "folderId")]
    pub folder_id: Option<i64>,
    #[serde(rename = "folderUid")]
    pub folder_uid: Option<String>,
    #[serde(rename = "folderTitle")]
    pub folder_title: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

impl GrafanaClient {
    pub fn new(
        base_url: String,
        service_account_token: Option<String>,
        username: Option<String>,
        password: Option<String>,
    ) -> Result<Self, SearcherError> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::ACCEPT,
            header::HeaderValue::from_static("application/json"),
        );

        if let Some(token) = service_account_token {
            let value = header::HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|e| SearcherError::Other(format!("invalid Grafana token: {e}")))?;
            headers.insert(header::AUTHORIZATION, value);
        } else if let (Some(username), Some(password)) = (username, password) {
            let value = header::HeaderValue::from_str(&format!(
                "Basic {}",
                basic_auth_encode(&username, &password)
            ))
            .map_err(|e| SearcherError::Other(format!("invalid Grafana auth header: {e}")))?;
            headers.insert(header::AUTHORIZATION, value);
        }

        let client = build_shared_http_client_with_headers(global_http_ssl_verify(), headers)?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn search(
        &self,
        query: Option<&str>,
        item_type: Option<&str>,
        tag: Option<&str>,
        folder_uid: Option<&str>,
        limit: Option<u32>,
        page: Option<u32>,
    ) -> Result<Vec<GrafanaSearchResult>, SearcherError> {
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(query) = query.filter(|v| !v.is_empty()) {
            params.push(("query", query.to_string()));
        }
        if let Some(item_type) = item_type.filter(|v| !v.is_empty()) {
            params.push(("type", item_type.to_string()));
        }
        if let Some(tag) = tag.filter(|v| !v.is_empty()) {
            params.push(("tag", tag.to_string()));
        }
        if let Some(folder_uid) = folder_uid.filter(|v| !v.is_empty()) {
            params.push(("folderUIDs", folder_uid.to_string()));
        }
        if let Some(limit) = limit {
            params.push(("limit", limit.to_string()));
        }
        if let Some(page) = page {
            params.push(("page", page.to_string()));
        }

        self.get_with_params("/api/search", &params).await
    }

    pub async fn list_datasources(
        &self,
        datasource_type: Option<&str>,
        limit: Option<usize>,
        page: Option<usize>,
    ) -> Result<Value, SearcherError> {
        let mut data: Vec<Value> = self.get("/api/datasources").await?;
        if let Some(datasource_type) = datasource_type.filter(|v| !v.is_empty()) {
            data.retain(|item| {
                item.get("type")
                    .and_then(Value::as_str)
                    .map(|value| value == datasource_type)
                    .unwrap_or(false)
            });
        }

        let total = data.len();
        let limit = limit.unwrap_or(total.max(1));
        let page = page.unwrap_or(1).max(1);
        let start = limit.saturating_mul(page.saturating_sub(1));
        let items: Vec<Value> = data.into_iter().skip(start).take(limit).collect();

        Ok(json!({
            "datasources": items,
            "total": total,
            "page": page,
            "limit": limit,
        }))
    }

    pub async fn get_datasource(
        &self,
        uid: Option<&str>,
        name: Option<&str>,
    ) -> Result<Value, SearcherError> {
        if let Some(uid) = uid.filter(|v| !v.is_empty()) {
            return self.get(&format!("/api/datasources/uid/{uid}")).await;
        }
        if let Some(name) = name.filter(|v| !v.is_empty()) {
            return self.get(&format!("/api/datasources/name/{name}")).await;
        }
        Err(SearcherError::Other(
            "either uid or name must be provided".to_string(),
        ))
    }

    pub async fn get_dashboard_by_uid(&self, uid: &str) -> Result<Value, SearcherError> {
        self.get(&format!("/api/dashboards/uid/{uid}")).await
    }

    pub async fn update_dashboard(&self, body: Value) -> Result<Value, SearcherError> {
        self.post("/api/dashboards/db", body).await
    }

    pub async fn create_folder(
        &self,
        title: &str,
        uid: Option<&str>,
    ) -> Result<Value, SearcherError> {
        let mut body = json!({ "title": title });
        if let Some(uid) = uid.filter(|v| !v.is_empty()) {
            body["uid"] = Value::String(uid.to_string());
        }
        self.post("/api/folders", body).await
    }

    pub async fn get_annotations(
        &self,
        dashboard_uid: Option<&str>,
        panel_id: Option<i64>,
        from: Option<&str>,
        to: Option<&str>,
        tags: &[String],
        limit: Option<u32>,
    ) -> Result<Value, SearcherError> {
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(uid) = dashboard_uid.filter(|v| !v.is_empty()) {
            params.push(("dashboardUID", uid.to_string()));
        }
        if let Some(panel_id) = panel_id {
            params.push(("panelId", panel_id.to_string()));
        }
        if let Some(from) = from.filter(|v| !v.is_empty()) {
            params.push(("from", from.to_string()));
        }
        if let Some(to) = to.filter(|v| !v.is_empty()) {
            params.push(("to", to.to_string()));
        }
        for tag in tags {
            params.push(("tags", tag.clone()));
        }
        if let Some(limit) = limit {
            params.push(("limit", limit.to_string()));
        }
        self.get_with_params("/api/annotations", &params).await
    }

    pub async fn create_annotation(&self, body: Value) -> Result<Value, SearcherError> {
        self.post("/api/annotations", body).await
    }

    pub async fn update_annotation(&self, id: i64, body: Value) -> Result<Value, SearcherError> {
        self.patch(&format!("/api/annotations/{id}"), body).await
    }

    pub async fn get_annotation_tags(
        &self,
        tag: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Value, SearcherError> {
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(tag) = tag.filter(|v| !v.is_empty()) {
            params.push(("tag", tag.to_string()));
        }
        if let Some(limit) = limit {
            params.push(("limit", limit.to_string()));
        }
        self.get_with_params("/api/annotations/tags", &params).await
    }

    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, SearcherError> {
        self.request_json(Method::GET, path, None, &[]).await
    }

    async fn get_with_params<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<T, SearcherError> {
        self.request_json(Method::GET, path, None, params).await
    }

    async fn post<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: Value,
    ) -> Result<T, SearcherError> {
        self.request_json(Method::POST, path, Some(body), &[]).await
    }

    async fn patch<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: Value,
    ) -> Result<T, SearcherError> {
        self.request_json(Method::PATCH, path, Some(body), &[]).await
    }

    async fn request_json<T: for<'de> Deserialize<'de>>(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        params: &[(&str, String)],
    ) -> Result<T, SearcherError> {
        let url = format!("{}{}", self.base_url, path);
        let mut request = self.client.request(method.clone(), &url);
        if !params.is_empty() {
            request = request.query(params);
        }
        if let Some(body) = body {
            request = request
                .header(header::CONTENT_TYPE, "application/json")
                .json(&body);
        }

        let response = request.send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SearcherError::ApiError(format!(
                "{} {} failed: {} - {}",
                method, url, status, error_text
            )));
        }

        response.json().await.map_err(SearcherError::RequestError)
    }
}

fn basic_auth_encode(username: &str, password: &str) -> String {
    use base64::{Engine as _, engine::general_purpose};
    general_purpose::STANDARD.encode(format!("{username}:{password}"))
}


#[cfg(test)]
mod tests {
    use super::*;

    fn live_client() -> Option<GrafanaClient> {
        let url = std::env::var("GRAFANA_URL").ok()?;
        let token = std::env::var("GRAFANA_SERVICE_ACCOUNT_TOKEN")
            .ok()
            .or_else(|| std::env::var("GRAFANA_TOKEN").ok());
        let username = std::env::var("GRAFANA_USERNAME").ok();
        let password = std::env::var("GRAFANA_PASSWORD").ok();
        if token.is_none() && (username.is_none() || password.is_none()) {
            return None;
        }
        GrafanaClient::new(url, token, username, password).ok()
    }

    #[tokio::test]
    async fn grafana_live_basic_read_apis_with_env_credentials() {
        let Some(client) = live_client() else {
            eprintln!("skipping live Grafana test; set GRAFANA_URL and credentials");
            return;
        };

        let dashboards = client
            .search(None, Some("dash-db"), None, None, Some(3), None)
            .await
            .expect("search dashboards should succeed");
        assert!(!dashboards.is_empty(), "expected at least one dashboard");

        let datasources = client
            .list_datasources(None, Some(20), Some(1))
            .await
            .expect("list datasources should succeed");
        assert!(
            datasources["datasources"].as_array().is_some_and(|items| !items.is_empty()),
            "expected at least one datasource"
        );

        let uid = dashboards
            .iter()
            .find_map(|dashboard| dashboard.uid.as_deref())
            .expect("dashboard search result should include uid");
        let dashboard = client
            .get_dashboard_by_uid(uid)
            .await
            .expect("get dashboard by uid should succeed");
        assert_eq!(dashboard["dashboard"]["uid"], uid);
    }
}

