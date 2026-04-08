use kube::{
    api::{Api, ListParams},
    config::Kubeconfig,
    Client as KubeClient,
};
use k8s_openapi::api::core::v1::{Event, Namespace, Pod};
use serde::{Deserialize, Serialize};
use crate::searcher::SearcherError;

/// Kubernetes API 客户端
pub struct KubernetesClient {
    client: KubeClient,
    context_name: Option<String>,
}

impl KubernetesClient {
    /// 创建新的 Kubernetes 客户端
    pub async fn new(context_name: Option<String>) -> Result<Self, SearcherError> {
        let client = KubeClient::try_default()
            .await
            .map_err(|e| SearcherError::Other(format!("Failed to create client: {}", e)))?;

        Ok(Self {
            client,
            context_name,
        })
    }

    /// 获取所有可用的 contexts
    pub async fn list_contexts(&self) -> Result<KubeContexts, SearcherError> {
        let config_file = dirs::home_dir()
            .map(|h| h.join(".kube").join("config"))
            .ok_or_else(|| SearcherError::Other("Cannot find home directory".to_string()))?;

        let config_path = config_file.to_str()
            .ok_or_else(|| SearcherError::Other("Invalid config path".to_string()))?;

        let kubeconfig = Kubeconfig::read_from(config_path)
            .map_err(|e| SearcherError::Other(format!("Failed to read kubeconfig: {}", e)))?;

        let contexts = kubeconfig.contexts
            .into_iter()
            .map(|ctx| {
                let context = ctx.context.as_ref();
                KubeContext {
                    name: ctx.name.clone(),
                    cluster: context
                        .map(|c| c.cluster.clone())
                        .unwrap_or_default(),
                    user: context
                        .and_then(|c| c.user.clone())
                        .unwrap_or_default(),
                }
            })
            .collect();

        Ok(KubeContexts { contexts })
    }

    /// 获取当前 kubeconfig 内容
    pub async fn get_config(&self, minified: bool) -> Result<String, SearcherError> {
        let config_file = dirs::home_dir()
            .map(|h| h.join(".kube").join("config"))
            .ok_or_else(|| SearcherError::Other("Cannot find home directory".to_string()))?;

        let config_path = config_file.to_str()
            .ok_or_else(|| SearcherError::Other("Invalid config path".to_string()))?;

        let kubeconfig = Kubeconfig::read_from(config_path)
            .map_err(|e| SearcherError::Other(format!("Failed to read kubeconfig: {}", e)))?;

        if minified {
            if let Some(current_ctx) = &kubeconfig.current_context {
                let current_context = kubeconfig.contexts.iter()
                    .find(|c| &c.name == current_ctx)
                    .map(|c| serde_json::to_value(&c).ok());

                let minified = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "Config",
                    "current-context": current_ctx,
                    "contexts": current_context,
                    "clusters": kubeconfig.clusters,
                    "auth_infos": kubeconfig.auth_infos
                });
                Ok(serde_json::to_string_pretty(&minified).unwrap_or_default())
            } else {
                Err(SearcherError::Other("No current context found".to_string()))
            }
        } else {
            Ok(serde_yaml::to_string(&kubeconfig)
                .map_err(|e| SearcherError::Other(format!("Failed to serialize config: {}", e)))?)
        }
    }

    /// 列出 Pod
    pub async fn list_pods(&self, namespace: Option<&str>) -> Result<KubePodList, SearcherError> {
        let api: Api<Pod> = if let Some(ns) = namespace {
            Api::namespaced(self.client.clone(), ns)
        } else {
            Api::all(self.client.clone())
        };

        let pod_list = api.list(&ListParams::default()).await
            .map_err(|e| SearcherError::Other(format!("Failed to list pods: {}", e)))?;

        Ok(KubePodList {
            items: pod_list.items,
            metadata: pod_list.metadata,
        })
    }

    /// 获取指定的 Pod
    pub async fn get_pod(&self, name: &str, namespace: Option<&str>) -> Result<Pod, SearcherError> {
        let api: Api<Pod> = namespace
            .map(|ns| Api::namespaced(self.client.clone(), ns))
            .unwrap_or_else(|| Api::default_namespaced(self.client.clone()));

        api.get(name).await
            .map_err(|e| SearcherError::Other(format!("Failed to get pod: {}", e)))
    }

    /// 删除 Pod
    pub async fn delete_pod(&self, name: &str, namespace: Option<&str>) -> Result<String, SearcherError> {
        let api: Api<Pod> = namespace
            .map(|ns| Api::namespaced(self.client.clone(), ns))
            .unwrap_or_else(|| Api::default_namespaced(self.client.clone()));

        let result = api.delete(name, &Default::default()).await
            .map_err(|e| SearcherError::Other(format!("Failed to delete pod: {}", e)))?;

        // Return the deletion status as JSON string
        serde_json::to_string(&result)
            .map_err(|e| SearcherError::Other(format!("Failed to serialize: {}", e)))
    }

    /// 列出命名空间
    pub async fn list_namespaces(&self) -> Result<KubeNamespaceList, SearcherError> {
        let api: Api<Namespace> = Api::all(self.client.clone());
        let ns_list = api.list(&ListParams::default()).await
            .map_err(|e| SearcherError::Other(format!("Failed to list namespaces: {}", e)))?;

        Ok(KubeNamespaceList {
            items: ns_list.items,
            metadata: ns_list.metadata,
        })
    }

    /// 列出事件
    pub async fn list_events(&self, namespace: Option<&str>) -> Result<KubeEventList, SearcherError> {
        let api: Api<Event> = if let Some(ns) = namespace {
            Api::namespaced(self.client.clone(), ns)
        } else {
            Api::all(self.client.clone())
        };

        let event_list = api.list(&ListParams::default()).await
            .map_err(|e| SearcherError::Other(format!("Failed to list events: {}", e)))?;

        Ok(KubeEventList {
            items: event_list.items,
            metadata: event_list.metadata,
        })
    }
}

// ========== 数据结构定义 ==========

/// Kubernetes contexts 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubeContexts {
    pub contexts: Vec<KubeContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubeContext {
    pub name: String,
    pub cluster: String,
    pub user: String,
}

/// Pod 列表响应（用于序列化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubePodList {
    pub items: Vec<Pod>,
    pub metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ListMeta,
}

/// Namespace 列表响应（用于序列化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubeNamespaceList {
    pub items: Vec<Namespace>,
    pub metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ListMeta,
}

/// Event 列表响应（用于序列化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubeEventList {
    pub items: Vec<Event>,
    pub metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ListMeta,
}
