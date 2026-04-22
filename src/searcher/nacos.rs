use crate::searcher::SearcherError;
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Nacos API 客户端
///
/// # Note
/// 需要 Nacos 3.0+ 版本，基于 Nacos Admin API v3
pub struct NacosClient {
    client: Client,
    base_url: String,
    access_token: Option<String>,
}

impl NacosClient {
    /// 创建新的 Nacos 客户端
    ///
    /// # 参数
    /// - `base_url`: Nacos 服务器的基础 URL (例如: http://localhost:8848)
    /// - `access_token`: 访问令牌（可选，从 /nacos/v3/auth/user/login 获取）
    pub fn new(base_url: String, access_token: Option<String>) -> Self {
        let client = Client::builder()
            .build()
            .expect("Failed to create HTTP client");

        // 移除末尾的斜杠
        let base_url = base_url.trim_end_matches('/').to_string();

        Self {
            client,
            base_url,
            access_token,
        }
    }

    /// 构建完整的 API URL
    fn build_url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// 发送 GET 请求
    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T, SearcherError> {
        let url = self.build_url(path);

        let mut request = self.client.get(&url);

        // 如果有 access_token，添加到请求头
        if let Some(token) = &self.access_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SearcherError::ApiError(format!(
                "GET {} failed: {} - {}",
                url, status, error_text
            )));
        }

        let data = response.json().await?;
        Ok(data)
    }

    /// 发送带查询参数的 GET 请求
    async fn get_with_params<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<T, SearcherError> {
        let url = self.build_url(path);

        let mut request = self.client.get(&url);

        // 添加查询参数
        for (key, value) in params {
            request = request.query(&[(key, value)]);
        }

        // 如果有 access_token，添加到请求头
        if let Some(token) = &self.access_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SearcherError::ApiError(format!(
                "GET {} failed: {} - {}",
                url, status, error_text
            )));
        }

        let data = response.json().await?;
        Ok(data)
    }

    // ========== Namespace API ==========

    /// 获取所有命名空间列表
    pub async fn list_namespaces(&self) -> Result<NamespacesResponse, SearcherError> {
        self.get("/nacos/v3/admin/core/namespace/list").await
    }

    // ========== Service API ==========

    /// 获取服务列表
    pub async fn list_services(
        &self,
        params: &ListServicesParams,
    ) -> Result<ServicesResponse, SearcherError> {
        let mut query_params: Vec<(&str, String)> = vec![
            ("pageNo", params.page_no.to_string()),
            ("pageSize", params.page_size.to_string()),
        ];

        if let Some(ns) = &params.namespace_id {
            query_params.push(("namespaceId", ns.clone()));
        }
        if let Some(group) = &params.group_name_param {
            query_params.push(("groupNameParam", group.clone()));
        }
        if let Some(service) = &params.service_name_param {
            query_params.push(("serviceNameParam", service.clone()));
        }
        if let Some(ignore) = params.ignore_empty_service {
            query_params.push(("ignoreEmptyService", ignore.to_string()));
        }
        if let Some(with_instances) = params.with_instances {
            query_params.push(("withInstances", with_instances.to_string()));
        }

        let params_refs: Vec<(&str, &str)> =
            query_params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get_with_params("/nacos/v3/admin/ns/service/list", &params_refs)
            .await
    }

    /// 获取服务详情
    pub async fn get_service(
        &self,
        params: &GetServiceParams,
    ) -> Result<ServiceDetail, SearcherError> {
        let mut query_params = vec![];

        if let Some(ns) = &params.namespace_id {
            query_params.push(("namespaceId", ns));
        }
        if let Some(group) = &params.group_name {
            query_params.push(("groupName", group));
        }
        query_params.push(("serviceName", &params.service_name));

        let params_refs: Vec<(&str, &str)> =
            query_params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get_with_params("/nacos/v3/admin/ns/service", &params_refs)
            .await
    }

    /// 获取服务实例列表
    pub async fn list_instances(
        &self,
        params: &ListInstancesParams,
    ) -> Result<InstancesResponse, SearcherError> {
        let mut query_params = vec![];

        if let Some(ns) = &params.namespace_id {
            query_params.push(("namespaceId", ns));
        }
        if let Some(group) = &params.group_name {
            query_params.push(("groupName", group));
        }
        query_params.push(("serviceName", &params.service_name));

        if let Some(cluster) = &params.cluster_name {
            query_params.push(("clusterName", cluster));
        }

        let params_refs: Vec<(&str, &str)> =
            query_params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get_with_params("/nacos/v3/admin/ns/instance/list", &params_refs)
            .await
    }

    /// 获取服务订阅者列表
    pub async fn list_service_subscribers(
        &self,
        params: &ListServiceSubscribersParams,
    ) -> Result<ServiceSubscribersResponse, SearcherError> {
        let mut query_params: Vec<(&str, String)> = vec![
            ("pageNo", params.page_no.to_string()),
            ("pageSize", params.page_size.to_string()),
        ];

        if let Some(ns) = &params.namespace_id {
            query_params.push(("namespaceId", ns.clone()));
        }
        if let Some(group) = &params.group_name {
            query_params.push(("groupName", group.clone()));
        }
        query_params.push(("serviceName", params.service_name.clone()));

        if let Some(aggregation) = params.aggregation {
            query_params.push(("aggregation", aggregation.to_string()));
        }

        let params_refs: Vec<(&str, &str)> =
            query_params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get_with_params("/nacos/v3/admin/ns/service/subscribers", &params_refs)
            .await
    }

    // ========== Configuration API ==========

    /// 获取配置列表
    pub async fn list_configs(
        &self,
        params: &ListConfigsParams,
    ) -> Result<ConfigsResponse, SearcherError> {
        let mut query_params: Vec<(&str, String)> = vec![
            ("pageNo", params.page_no.to_string()),
            ("pageSize", params.page_size.to_string()),
        ];

        if let Some(ns) = &params.namespace_id {
            query_params.push(("namespaceId", ns.clone()));
        }
        if let Some(group) = &params.group_name {
            query_params.push(("groupName", group.clone()));
        }
        if let Some(data_id) = &params.data_id {
            query_params.push(("dataId", data_id.clone()));
        }
        if let Some(config_type) = &params.config_type {
            query_params.push(("type", config_type.clone()));
        }
        if let Some(tags) = &params.config_tags {
            query_params.push(("configTags", tags.clone()));
        }
        if let Some(app) = &params.app_name {
            query_params.push(("appName", app.clone()));
        }
        if let Some(search) = &params.search {
            query_params.push(("search", search.clone()));
        }

        let params_refs: Vec<(&str, &str)> =
            query_params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get_with_params("/nacos/v3/admin/cs/config/list", &params_refs)
            .await
    }

    /// 获取配置详情
    pub async fn get_config(
        &self,
        params: &GetConfigParams,
    ) -> Result<ConfigDetail, SearcherError> {
        let mut query_params = vec![];

        if let Some(ns) = &params.namespace_id {
            query_params.push(("namespaceId", ns));
        }
        query_params.push(("groupName", &params.group_name));
        query_params.push(("dataId", &params.data_id));

        let params_refs: Vec<(&str, &str)> =
            query_params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get_with_params("/nacos/v3/admin/cs/config", &params_refs)
            .await
    }

    /// 获取配置历史列表
    pub async fn list_config_history(
        &self,
        params: &ListConfigHistoryParams,
    ) -> Result<ConfigHistoryResponse, SearcherError> {
        let mut query_params: Vec<(&str, String)> = vec![
            ("pageNo", params.page_no.to_string()),
            ("pageSize", params.page_size.to_string()),
        ];

        if let Some(ns) = &params.namespace_id {
            query_params.push(("namespaceId", ns.clone()));
        }
        query_params.push(("groupName", params.group_name.clone()));
        query_params.push(("dataId", params.data_id.clone()));

        let params_refs: Vec<(&str, &str)> =
            query_params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get_with_params("/nacos/v3/admin/cs/history/list", &params_refs)
            .await
    }

    /// 获取配置历史详情
    pub async fn get_config_history(
        &self,
        params: &GetConfigHistoryParams,
    ) -> Result<ConfigHistoryDetail, SearcherError> {
        let mut query_params: Vec<(&str, String)> = vec![];

        if let Some(ns) = &params.namespace_id {
            query_params.push(("namespaceId", ns.clone()));
        }
        query_params.push(("groupName", params.group_name.clone()));
        query_params.push(("dataId", params.data_id.clone()));

        if let Some(nid) = params.nid {
            query_params.push(("nid", nid.to_string()));
        }

        let params_refs: Vec<(&str, &str)> =
            query_params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get_with_params("/nacos/v3/admin/cs/history", &params_refs)
            .await
    }

    /// 获取配置监听器列表
    pub async fn list_config_listeners(
        &self,
        params: &ListConfigListenersParams,
    ) -> Result<ConfigListenersResponse, SearcherError> {
        let mut query_params: Vec<(&str, String)> = vec![];

        if let Some(ns) = &params.namespace_id {
            query_params.push(("namespaceId", ns.clone()));
        }
        query_params.push(("groupName", params.group_name.clone()));
        query_params.push(("dataId", params.data_id.clone()));

        if let Some(aggregation) = params.aggregation {
            query_params.push(("aggregation", aggregation.to_string()));
        }

        let params_refs: Vec<(&str, &str)> =
            query_params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get_with_params("/nacos/v3/admin/cs/config/listener", &params_refs)
            .await
    }

    /// 获取客户端监听的配置列表
    pub async fn list_listened_configs(
        &self,
        params: &ListListenedConfigsParams,
    ) -> Result<ListenedConfigsResponse, SearcherError> {
        let mut query_params: Vec<(&str, String)> = vec![("ip", params.ip.clone())];

        if let Some(ns) = &params.namespace_id {
            query_params.push(("namespaceId", ns.clone()));
        }
        if let Some(aggregation) = params.aggregation {
            query_params.push(("aggregation", aggregation.to_string()));
        }

        let params_refs: Vec<(&str, &str)> =
            query_params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.get_with_params("/nacos/v3/admin/cs/listener", &params_refs)
            .await
    }
}

// ========== 请求参数结构 ==========

/// 获取服务列表的参数
#[derive(Debug, Clone, Default)]
pub struct ListServicesParams {
    pub page_no: i32,
    pub page_size: i32,
    pub namespace_id: Option<String>,
    pub group_name_param: Option<String>,
    pub service_name_param: Option<String>,
    pub ignore_empty_service: Option<bool>,
    pub with_instances: Option<bool>,
}

/// 获取服务详情的参数
#[derive(Debug, Clone, Default)]
pub struct GetServiceParams {
    pub namespace_id: Option<String>,
    pub group_name: Option<String>,
    pub service_name: String,
}

/// 获取服务实例列表的参数
#[derive(Debug, Clone, Default)]
pub struct ListInstancesParams {
    pub namespace_id: Option<String>,
    pub group_name: Option<String>,
    pub service_name: String,
    pub cluster_name: Option<String>,
}

/// 获取服务订阅者列表的参数
#[derive(Debug, Clone, Default)]
pub struct ListServiceSubscribersParams {
    pub page_no: i32,
    pub page_size: i32,
    pub namespace_id: Option<String>,
    pub group_name: Option<String>,
    pub service_name: String,
    pub aggregation: Option<bool>,
}

/// 获取配置列表的参数
#[derive(Debug, Clone, Default)]
pub struct ListConfigsParams {
    pub page_no: i32,
    pub page_size: i32,
    pub namespace_id: Option<String>,
    pub group_name: Option<String>,
    pub data_id: Option<String>,
    pub config_type: Option<String>,
    pub config_tags: Option<String>,
    pub app_name: Option<String>,
    pub search: Option<String>,
}

/// 获取配置详情的参数
#[derive(Debug, Clone, Default)]
pub struct GetConfigParams {
    pub namespace_id: Option<String>,
    pub group_name: String,
    pub data_id: String,
}

/// 获取配置历史的参数
#[derive(Debug, Clone, Default)]
pub struct ListConfigHistoryParams {
    pub page_no: i32,
    pub page_size: i32,
    pub namespace_id: Option<String>,
    pub group_name: String,
    pub data_id: String,
}

/// 获取配置历史详情的参数
#[derive(Debug, Clone, Default)]
pub struct GetConfigHistoryParams {
    pub namespace_id: Option<String>,
    pub group_name: String,
    pub data_id: String,
    pub nid: Option<i64>,
}

/// 获取配置监听器的参数
#[derive(Debug, Clone, Default)]
pub struct ListConfigListenersParams {
    pub namespace_id: Option<String>,
    pub group_name: String,
    pub data_id: String,
    pub aggregation: Option<bool>,
}

/// 获取客户端监听配置的参数
#[derive(Debug, Clone, Default)]
pub struct ListListenedConfigsParams {
    pub namespace_id: Option<String>,
    pub ip: String,
    pub aggregation: Option<bool>,
}

// ========== 响应数据结构 ==========

/// 命名空间列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespacesResponse {
    pub namespaces: Vec<Namespace>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Namespace {
    pub namespace: String,
    pub namespace_show_name: String,
    pub namespace_id: String,
    pub namespace_desc: Option<String>,
    pub quota: Option<i32>,
    pub namespace_type: Option<i32>,
}

/// 服务列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicesResponse {
    pub count: i32,
    pub doms: Vec<String>,
    pub service_detail_infos: Option<Vec<ServiceDetailInfo>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDetailInfo {
    pub name: String,
    pub group_name: String,
    pub namespace_id: String,
    pub metadata: Option<serde_json::Value>,
    pub instances: Option<Vec<Instance>>,
}

/// 服务详情
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDetail {
    pub name: String,
    pub group_name: String,
    pub namespace_id: String,
    pub metadata: Option<serde_json::Value>,
    pub clusters: Option<Vec<Cluster>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    pub name: String,
    pub health_checker: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

/// 实例列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstancesResponse {
    pub hosts: Vec<Instance>,
    pub dom: String,
    pub cache_millis: i32,
    pub checksum: String,
    pub last_ref_time: i64,
    pub use_specified_url: bool,
    env: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub instance_id: String,
    pub ip: String,
    pub port: i32,
    pub weight: f64,
    pub healthy: bool,
    pub enabled: bool,
    pub ephemeral: bool,
    pub cluster_name: Option<String>,
    pub service_name: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub instance_heart_beat_time: Option<i64>,
    pub instance_heart_beat_interval: Option<i64>,
    pub ip_delete_timeout: Option<i64>,
}

/// 服务订阅者响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSubscribersResponse {
    pub subscribers: Vec<Subscriber>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscriber {
    pub addr: String,
    pub app: String,
    pub ip: String,
    pub port: i32,
    pub client_version: Option<String>,
    pub service: Option<ServiceInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInfo {
    pub name: String,
    pub group_name: String,
    pub namespace_id: String,
}

/// 配置列表响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigsResponse {
    pub page_items: Vec<ConfigItem>,
    pub total: i32,
    pub page_number: i32,
    pub pages_available: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigItem {
    pub id: Option<i64>,
    pub data_id: String,
    pub group: String,
    pub content: Option<String>,
    pub tenant: String,
    pub app_name: Option<String>,
    pub description: Option<String>,
    pub md5: Option<String>,
    pub tags: Option<Vec<String>>,
    pub create_time: Option<i64>,
    pub create_user: Option<String>,
    pub update_time: Option<i64>,
    pub update_user: Option<String>,
    pub encrypted_data_key: Option<String>,
    pub encrypted_data: Option<String>,
    pub type_: Option<String>, // type 是 Rust 关键字，使用 type_
}

/// 配置详情
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDetail {
    pub data_id: String,
    pub group: String,
    pub content: String,
    pub tenant: String,
    pub app_name: Option<String>,
    pub description: Option<String>,
    pub md5: String,
    pub tags: Option<Vec<String>>,
    pub create_time: i64,
    pub create_user: Option<String>,
    pub update_time: i64,
    pub update_user: Option<String>,
    pub encrypted_data_key: Option<String>,
    pub encrypted_data: Option<String>,
    pub type_: String,
}

/// 配置历史响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigHistoryResponse {
    pub page_items: Vec<ConfigHistoryItem>,
    pub total: i32,
    pub page_number: i32,
    pub pages_available: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryItem {
    pub id: i64,
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub app_name: Option<String>,
    pub type_: Option<String>,
    pub op_type: String,
    pub create_time: i64,
    pub create_user: Option<String>,
}

/// 配置历史详情
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigHistoryDetail {
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub app_name: Option<String>,
    pub type_: String,
    pub content: String,
    pub md5: String,
    pub op_type: String,
    pub create_time: i64,
    pub create_user: Option<String>,
    pub last_id: Option<i64>,
}

/// 配置监听器响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigListenersResponse {
    pub listeners: Vec<ConfigListener>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigListener {
    pub ip: String,
    pub port: i32,
    pub client_version: Option<String>,
    pub app: Option<String>,
    pub md5: Option<String>,
    pub config_info: Option<ConfigInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInfo {
    pub data_id: String,
    pub group: String,
    pub tenant: String,
    pub md5: String,
}

/// 客户端监听的配置响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListenedConfigsResponse {
    pub configs: Vec<ListenedConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenedConfig {
    pub tenant: String,
    pub group: String,
    pub data_id: String,
    pub md5: String,
    pub tenant_id: Option<String>,
}
