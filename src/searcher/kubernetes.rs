use kube::{
    api::{Api, ListParams},
    config::Kubeconfig,
    Client as KubeClient,
};
use k8s_openapi::api::{
    core::v1::{Event, EventList, Namespace, NamespaceList, Pod, PodList},
};
use serde::{Deserialize, Serialize};
use crate::searcher::SearcherError;

/// Kubernetes API 客户端
pub struct KubernetesClient {
    client: Client,
    context_name: Option<String>,
}

impl KubernetesClient {
    /// 创建新的 Kubernetes 客户端
    pub async fn new(context_name: Option<String>) -> Result<Self, SearcherError> {
        let client = Client::try_default()
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
            .map(|(name, ctx)| KubeContext {
                name: name.clone(),
                cluster: ctx.cluster.unwrap_or_default(),
                user: ctx.user.unwrap_or_default(),
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
                let minified = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "Config",
                    "contexts": kubeconfig.contexts.get(current_ctx),
                    "current-context": current_ctx,
                    "clusters": kubeconfig.clusters,
                    "users": kubeconfig.users
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
    pub async fn list_pods(&self, namespace: Option<&str>) -> Result<PodList, SearcherError> {
        let api: Api<Pod> = if let Some(ns) = namespace {
            Api::namespaced(self.client.clone(), ns)
        } else {
            Api::all(self.client.clone())
        };

        api.list(&ListParams::default()).await
            .map_err(|e| SearcherError::Other(format!("Failed to list pods: {}", e)))
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

        api.delete(name, &Default::default()).await
            .map_err(|e| SearcherError::Other(format!("Failed to delete pod: {}", e)))
            .and_then(|either| {
                either.map_left(|p| serde_json::to_string(&p).unwrap_or_default())
                    .map_err(|e| SearcherError::Other(format!("Failed to serialize: {}", e)))
            })
    }

    /// 列出命名空间
    pub async fn list_namespaces(&self) -> Result<NamespaceList, SearcherError> {
        let api: Api<Namespace> = Api::all(self.client.clone());
        api.list(&ListParams::default()).await
            .map_err(|e| SearcherError::Other(format!("Failed to list namespaces: {}", e)))
    }

    /// 列出事件
    pub async fn list_events(&self, namespace: Option<&str>) -> Result<EventList, SearcherError> {
        let api: Api<Event> = if let Some(ns) = namespace {
            Api::namespaced(self.client.clone(), ns)
        } else {
            Api::all(self.client.clone())
        };

        api.list(&ListParams::default()).await
            .map_err(|e| SearcherError::Other(format!("Failed to list events: {}", e)))
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
