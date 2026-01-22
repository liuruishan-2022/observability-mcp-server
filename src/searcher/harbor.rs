use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::searcher::SearcherError;

/// Harbor API 客户端
pub struct HarborClient {
    client: Client,
    base_url: String,
    auth_header: String,
}

impl HarborClient {
    /// 创建新的 Harbor 客户端
    ///
    /// # 参数
    /// - `base_url`: Harbor 服务器的基础 URL (例如: https://harbor.example.com)
    /// - `username`: Harbor 用户名
    /// - `password`: Harbor 密码
    pub fn new(base_url: String, username: String, password: String) -> Self {
        let auth_header = format!("Basic {}", basic_auth_encode(&username, &password));

        let client = Client::builder()
            .build()
            .expect("Failed to create HTTP client");

        // 移除末尾的斜杠
        let base_url = base_url.trim_end_matches('/').to_string();

        Self {
            client,
            base_url,
            auth_header,
        }
    }

    /// 构建完整的 API URL
    fn build_url(&self, path: &str) -> String {
        format!("{}{}{}", self.base_url, "/api/v2.0", path)
    }

    /// 发送 GET 请求
    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, SearcherError> {
        let url = self.build_url(path);
        let response = self
            .client
            .get(&url)
            .header(header::AUTHORIZATION, &self.auth_header)
            .header(header::ACCEPT, "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SearcherError::ApiError(
                format!("GET {} failed: {} - {}", url, status, error_text)
            ));
        }

        let data = response.json().await?;
        Ok(data)
    }

    /// 发送 DELETE 请求
    async fn delete(&self, path: &str) -> Result<(), SearcherError> {
        let url = self.build_url(path);
        let response = self
            .client
            .delete(&url)
            .header(header::AUTHORIZATION, &self.auth_header)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SearcherError::ApiError(
                format!("DELETE {} failed: {} - {}", url, status, error_text)
            ));
        }

        Ok(())
    }

    /// 发送 POST 请求
    async fn post<T: for<'de> Deserialize<'de>, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, SearcherError> {
        let url = self.build_url(path);
        let response = self
            .client
            .post(&url)
            .header(header::AUTHORIZATION, &self.auth_header)
            .header(header::CONTENT_TYPE, "application/json")
            .json(body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SearcherError::ApiError(
                format!("POST {} failed: {} - {}", url, status, error_text)
            ));
        }

        let data = response.json().await?;
        Ok(data)
    }

    /// 获取所有项目
    pub async fn get_projects(&self) -> Result<Vec<Project>, SearcherError> {
        self.get("/projects").await
    }

    /// 获取指定项目的信息
    ///
    /// # 参数
    /// - `project_id_or_name`: 项目 ID 或项目名称
    pub async fn get_project(&self, project_id_or_name: &str) -> Result<Project, SearcherError> {
        self.get(&format!("/projects/{}", project_id_or_name)).await
    }

    /// 创建项目
    pub async fn create_project(&self, request: &CreateProjectRequest) -> Result<Project, SearcherError> {
        self.post("/projects", request).await
    }

    /// 删除项目
    ///
    /// # 参数
    /// - `project_id_or_name`: 项目 ID 或项目名称
    pub async fn delete_project(&self, project_id_or_name: &str) -> Result<(), SearcherError> {
        self.delete(&format!("/projects/{}", project_id_or_name)).await
    }

    /// 获取项目的仓库列表
    ///
    /// # 参数
    /// - `project_id_or_name`: 项目 ID 或项目名称
    pub async fn get_repositories(&self, project_id_or_name: &str) -> Result<Vec<Repository>, SearcherError> {
        self.get(&format!("/projects/{}/repositories", project_id_or_name)).await
    }

    /// 删除仓库
    ///
    /// # 参数
    /// - `project_id_or_name`: 项目 ID 或项目名称
    /// - `repository_name`: 仓库名称
    pub async fn delete_repository(&self, project_id_or_name: &str, repository_name: &str) -> Result<(), SearcherError> {
        self.delete(&format!("/projects/{}/repositories/{}", project_id_or_name, repository_name)).await
    }

    /// 获取仓库的标签列表
    ///
    /// # 参数
    /// - `project_id_or_name`: 项目 ID 或项目名称
    /// - `repository_name`: 仓库名称
    pub async fn get_artifacts(&self, project_id_or_name: &str, repository_name: &str) -> Result<Vec<Artifact>, SearcherError> {
        self.get(&format!(
            "/projects/{}/repositories/{}/artifacts",
            project_id_or_name, repository_name
        )).await
    }

    /// 删除标签
    ///
    /// # 参数
    /// - `project_id_or_name`: 项目 ID 或项目名称
    /// - `repository_name`: 仓库名称
    /// - `digest`: artifact digest
    pub async fn delete_artifact(&self, project_id_or_name: &str, repository_name: &str, digest: &str) -> Result<(), SearcherError> {
        self.delete(&format!(
            "/projects/{}/repositories/{}/artifacts/{}",
            project_id_or_name, repository_name, digest
        )).await
    }

    /// 获取项目的 Helm Charts
    ///
    /// # 参数
    /// - `project_id_or_name`: 项目 ID 或项目名称
    pub async fn get_helm_charts(&self, project_id_or_name: &str) -> Result<Vec<HelmChart>, SearcherError> {
        self.get(&format!("/projects/{}/helm/charts", project_id_or_name)).await
    }

    /// 获取 Helm Chart 的版本列表
    ///
    /// # 参数
    /// - `project_id_or_name`: 项目 ID 或项目名称
    /// - `chart_name`: Chart 名称
    pub async fn get_helm_chart_versions(&self, project_id_or_name: &str, chart_name: &str) -> Result<Vec<HelmChartVersion>, SearcherError> {
        self.get(&format!(
            "/projects/{}/helm/charts/{}/versions",
            project_id_or_name, chart_name
        )).await
    }

    /// 删除 Helm Chart 版本
    ///
    /// # 参数
    /// - `project_id_or_name`: 项目 ID 或项目名称
    /// - `chart_name`: Chart 名称
    /// - `version`: Chart 版本
    pub async fn delete_helm_chart_version(
        &self,
        project_id_or_name: &str,
        chart_name: &str,
        version: &str,
    ) -> Result<(), SearcherError> {
        self.delete(&format!(
            "/projects/{}/helm/charts/{}/versions/{}",
            project_id_or_name, chart_name, version
        )).await
    }
}

/// Base64 编码用于基本认证
fn basic_auth_encode(username: &str, password: &str) -> String {
    let credentials = format!("{}:{}", username, password);
    use base64::prelude::*;
    BASE64_STANDARD.encode(credentials)
}

// ========== 数据结构定义 ==========

/// Harbor 项目
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub project_id: i64,
    pub name: String,
    pub owner_id: Option<i64>,
    pub creation_time: String,
    pub update_time: Option<String>,
    pub deleted: bool,
    pub owner_name: Option<String>,
    pub repo_count: Option<i64>,
    pub metadata: Option<ProjectMetadata>,
    pub cve_whitelist: Option<CveWhitelist>,
    pub registry_id: Option<i64>,
    pub public: bool,
}

/// 项目元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub public: Option<String>,
    pub enable_content_trust: Option<String>,
    pub prevent_vul: Option<String>,
    pub severity: Option<String>,
    pub auto_scan: Option<String>,
    pub retention_id: Option<String>,
}

/// CVE 白名单
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CveWhitelist {
    pub items: Vec<CveWhitelistItem>,
    pub project_id: Option<i64>,
    pub id: Option<i64>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CveWhitelistItem {
    pub cve_id: Option<String>,
}

/// 创建项目请求
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectRequest {
    pub project_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ProjectMetadata>,
}

/// 仓库信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub id: Option<i64>,
    pub name: String,
    pub project_id: Option<i64>,
    pub description: Option<String>,
    pub pull_count: Option<i64>,
    pub artifact_count: Option<i64>,
    pub creation_time: String,
    pub update_time: Option<String>,
}

/// Artifact 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub digest: String,
    pub id: Option<i64>,
    pub project_id: Option<i64>,
    pub repository_id: Option<i64>,
    pub pull_time: Option<String>,
    pub push_time: Option<String>,
    pub tags: Vec<ArtifactTag>,
    pub size: Option<i64>,
    pub manifest_media_type: Option<String>,
    pub configuration: Option<ArtifactConfiguration>,
    pub references: Vec<ArtifactReference>,
    pub annotations: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactTag {
    pub artifact_id: Option<i64>,
    pub id: Option<i64>,
    pub name: String,
    pub push_time: Option<String>,
    pub pull_time: Option<String>,
    pub repository_id: Option<i64>,
    pub signed: Option<bool>,
    pub immutable: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactConfiguration {
    pub digest: Option<String>,
    pub created: Option<String>,
    pub architecture: Option<String>,
    pub os: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactReference {
    pub parent_digest: Option<String>,
    pub child_digest: Option<String>,
    pub references: Vec<ArtifactReferenceInner>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactReferenceInner {
    pub parent_digest: Option<String>,
    pub child_digest: Option<String>,
    pub type_: Option<String>,
    pub source_type: Option<String>,
    pub annotations: Option<HashMap<String, String>>,
}

/// Helm Chart 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelmChart {
    pub name: String,
    pub created: String,
    pub updated: Option<String>,
    pub home: Option<String>,
    pub sources: Vec<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub keywords: Vec<String>,
    pub maintainers: Vec<ChartMaintainer>,
    pub icon: Option<String>,
    pub deprecated: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartMaintainer {
    pub name: Option<String>,
    pub email: Option<String>,
}

/// Helm Chart 版本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelmChartVersion {
    pub version: String,
    pub created: String,
    pub updated: Option<String>,
    pub signature: Option<String>,
    pub icon: Option<String>,
    pub app_version: Option<String>,
    pub api_version: Option<String>,
    pub sources: Vec<String>,
    pub description: Option<String>,
    pub digest: Option<String>,
    pub home: Option<String>,
    pub keywords: Vec<String>,
    pub maintainers: Vec<ChartMaintainer>,
    pub deprecated: Option<bool>,
    pub labels: HashMap<String, String>,
    pub urls: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_auth_encode() {
        let encoded = basic_auth_encode("admin", "Harbor12345");
        // 验证编码结果
        assert!(!encoded.is_empty());
    }
}
