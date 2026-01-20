use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo, ToolsCapability},
    schemars::JsonSchema,
    service::{NotificationContext, RequestContext},
    tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::docs::SharedDocsLoader;
use crate::searcher::{self, Searcher};
use std::sync::Arc;

///
/// 放置所有的tools的地方
///

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DocsReadRequest {
    #[schemars(description = "要读取的文档文件名")]
    pub file: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DocsSearchRequest {
    #[schemars(description = "搜索查询")]
    pub query: String,
    #[schemars(description = "返回的最大结果数量")]
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct QueryExemplarsRequest {
    #[schemars(description = "PromQL 查询语句")]
    query: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct LabelValuesRequest {
    #[schemars(description = "标签名称")]
    label_name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct QueryRequest {
    #[schemars(description = "PromQL 查询语句")]
    query: String,
    #[schemars(description = "可选的时间戳，Unix时间戳或RFC3339格式")]
    time: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct QueryRangeRequest {
    #[schemars(description = "PromQL 查询语句")]
    query: String,
    #[schemars(description = "开始时间戳，Unix时间戳或RFC3339格式")]
    start: String,
    #[schemars(description = "结束时间戳，Unix时间戳或RFC3339格式")]
    end: String,
    #[schemars(description = "查询步长，例如: 15s, 1m, 1h")]
    step: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct SeriesRequest {
    #[schemars(description = "��序选择器列表，例如: [\"up\", \"process_cpu_seconds_total\"]")]
    matches: Vec<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct TargetsMetadataRequest {
    #[schemars(description = "指标名称，可选")]
    metric: Option<String>,
    #[schemars(description = "标签名称，可选")]
    label: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct LokiQueryRequest {
    #[schemars(description = "LogQL 查询语句")]
    query: String,
    #[schemars(description = "开始时间，RFC3339格式或纳秒时间戳")]
    start: String,
    #[schemars(description = "结束时间，RFC3339格式或纳秒时间戳")]
    end: String,
    #[schemars(description = "返回的最大条目数")]
    limit: Option<u32>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct LokiLabelsRequest {
    #[schemars(description = "开始时间，可选")]
    start: Option<String>,
    #[schemars(description = "结束时间，可选")]
    end: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct LokiLabelValuesRequest {
    #[schemars(description = "标签名称")]
    label_name: String,
    #[schemars(description = "开始时间，可选")]
    start: Option<String>,
    #[schemars(description = "结束时间，可选")]
    end: Option<String>,
}

pub struct Tools {
    tool_router: ToolRouter<Tools>,
    searcher: Searcher,
    docs_loader: SharedDocsLoader,
}

#[tool_router(router = tool_router)]
impl Tools {
    pub fn new() -> Self {
        Tools {
            tool_router: Self::tool_router(),
            searcher: searcher::build_searcher().expect("build searcher error."),
            docs_loader: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    pub fn with_docs_loader(mut self, docs_loader: SharedDocsLoader) -> Self {
        self.docs_loader = docs_loader;
        self
    }

    #[tool(description = "获取当前服务的版本号")]
    pub async fn version(&self) -> String {
        info!("获取当前服务的版本号!");
        "v1.0.0".to_string()
    }

    #[tool(description = "获取Prometheus的构建信息")]
    pub async fn prom_build_info(&self) -> String {
        info!("获取Prometheus的构建信息!");
        match self.searcher.prometheus.build_info().await {
            Ok(info) => {
                serde_json::to_string(&info).unwrap_or_else(|_| "Failed to serialize".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "获取AlertManager列表")]
    pub async fn prom_alert_managers(&self) -> String {
        info!("获取AlertManager列表!");
        match self.searcher.prometheus.alert_managers().await {
            Ok(managers) => serde_json::to_string(&managers)
                .unwrap_or_else(|_| "Failed to serialize".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "清理TSDB中的墓碑记录(tombstones)，回收磁盘空间")]
    pub async fn prom_clean_tombstones(&self) -> String {
        info!("清理TSDB墓碑记录!");
        match self.searcher.prometheus.clean_tombstones().await {
            Ok(msg) => msg,
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "删除匹配选择器的时序数据")]
    pub async fn prom_delete_series(
        &self,
        Parameters(params): Parameters<SeriesRequest>,
    ) -> String {
        info!("删除时序数据，匹配: {:?}", params.matches);
        match self
            .searcher
            .prometheus
            .delete_series(&params.matches)
            .await
        {
            Ok(msg) => msg,
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "获取Prometheus的当前配置")]
    pub async fn prom_config(&self) -> String {
        info!("获取Prometheus配置!");
        match self.searcher.prometheus.config().await {
            Ok(config) => {
                serde_json::to_string(&config).unwrap_or_else(|_| "Failed to serialize".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "查询与给定PromQL查询匹配的exemplars")]
    pub async fn prom_query_exemplars(
        &self,
        Parameters(params): Parameters<QueryExemplarsRequest>,
    ) -> String {
        info!("查询exemplars: {}", params.query);
        match self
            .searcher
            .prometheus
            .query_exemplars(&params.query)
            .await
        {
            Ok(exemplars) => serde_json::to_string(&exemplars)
                .unwrap_or_else(|_| "Failed to serialize".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "获取Prometheus的命令行启动标志")]
    pub async fn prom_flags(&self) -> String {
        info!("获取Prometheus命令行标志!");
        match self.searcher.prometheus.flags().await {
            Ok(flags) => {
                serde_json::to_string(&flags).unwrap_or_else(|_| "Failed to serialize".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "获取Prometheus中的所有标签名")]
    pub async fn prom_labels(&self) -> String {
        info!("获取Prometheus标签列表!");
        match self.searcher.prometheus.labels().await {
            Ok(labels) => {
                serde_json::to_string(&labels).unwrap_or_else(|_| "Failed to serialize".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "获取指定标签名的所有可能值")]
    pub async fn prom_label_values(
        &self,
        Parameters(params): Parameters<LabelValuesRequest>,
    ) -> String {
        info!("获取标签 {} 的值列表", params.label_name);
        match self
            .searcher
            .prometheus
            .label_values(&params.label_name)
            .await
        {
            Ok(values) => {
                serde_json::to_string(&values).unwrap_or_else(|_| "Failed to serialize".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "获取当前活动的告警列表")]
    pub async fn prom_alerts(&self) -> String {
        info!("获取告警列表!");
        match self.searcher.prometheus.alerts().await {
            Ok(alerts) => {
                serde_json::to_string(&alerts).unwrap_or_else(|_| "Failed to serialize".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "获取Prometheus的所有规则(记录规则和告警规则)")]
    pub async fn prom_rules(&self) -> String {
        info!("获取Prometheus规则列表!");
        match self.searcher.prometheus.rules().await {
            Ok(rules) => {
                serde_json::to_string(&rules).unwrap_or_else(|_| "Failed to serialize".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "获取Prometheus中所有指标的元数据")]
    pub async fn prom_metadata(&self) -> String {
        info!("获取Prometheus指标元数据!");
        match self.searcher.prometheus.metadata().await {
            Ok(metadata) => serde_json::to_string(&metadata)
                .unwrap_or_else(|_| "Failed to serialize".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "执行PromQL即时查询")]
    pub async fn prom_query(&self, Parameters(params): Parameters<QueryRequest>) -> String {
        info!("执行PromQL查询: {}", params.query);
        match self
            .searcher
            .prometheus
            .query(&params.query, params.time.as_deref())
            .await
        {
            Ok(result) => {
                serde_json::to_string(&result).unwrap_or_else(|_| "Failed to serialize".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "执行PromQL范围查询")]
    pub async fn prom_query_range(
        &self,
        Parameters(params): Parameters<QueryRangeRequest>,
    ) -> String {
        info!(
            "执行PromQL范围查询: {} ({} to {}, step: {})",
            params.query, params.start, params.end, params.step
        );
        match self
            .searcher
            .prometheus
            .query_range(&params.query, &params.start, &params.end, &params.step)
            .await
        {
            Ok(result) => {
                serde_json::to_string(&result).unwrap_or_else(|_| "Failed to serialize".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "获取Prometheus的运行时环境信息")]
    pub async fn prom_runtime_info(&self) -> String {
        info!("获取Prometheus运行时信息!");
        match self.searcher.prometheus.runtime_info().await {
            Ok(info) => {
                serde_json::to_string(&info).unwrap_or_else(|_| "Failed to serialize".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "查询匹配选择器的时序")]
    pub async fn prom_series(&self, Parameters(params): Parameters<SeriesRequest>) -> String {
        info!("查询时序，匹配: {:?}", params.matches);
        match self.searcher.prometheus.series(&params.matches).await {
            Ok(series) => {
                serde_json::to_string(&series).unwrap_or_else(|_| "Failed to serialize".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "创建TSDB快照")]
    pub async fn prom_snapshot(&self) -> String {
        info!("创建TSDB快照!");
        match self.searcher.prometheus.snapshot().await {
            Ok(snapshot) => serde_json::to_string(&snapshot)
                .unwrap_or_else(|_| "Failed to serialize".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "获取targets的指标元数据")]
    pub async fn prom_targets_metadata(
        &self,
        Parameters(params): Parameters<TargetsMetadataRequest>,
    ) -> String {
        info!("获取targets元数据!");
        match self
            .searcher
            .prometheus
            .targets_metadata(params.metric.as_deref(), params.label.as_deref())
            .await
        {
            Ok(metadata) => serde_json::to_string(&metadata)
                .unwrap_or_else(|_| "Failed to serialize".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "获取所有targets的信息")]
    pub async fn prom_targets(&self) -> String {
        info!("获取targets列表!");
        match self.searcher.prometheus.targets().await {
            Ok(targets) => serde_json::to_string(&targets)
                .unwrap_or_else(|_| "Failed to serialize".to_string()),
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "获取TSDB的状态统计信息")]
    pub async fn prom_tsdb_status(&self) -> String {
        info!("获取TSDB状态!");
        match self.searcher.prometheus.tsdb_status().await {
            Ok(status) => {
                serde_json::to_string(&status).unwrap_or_else(|_| "Failed to serialize".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "获取WAL重放的状态信息")]
    pub async fn prom_wal_replay(&self) -> String {
        info!("获取WAL重放状态!");
        match self.searcher.prometheus.wal_replay().await {
            Ok(replay) => {
                serde_json::to_string(&replay).unwrap_or_else(|_| "Failed to serialize".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "健康检查，确认Prometheus服务是否健康")]
    pub async fn prom_healthy(&self) -> String {
        info!("执行健康检查!");
        match self.searcher.prometheus.healthy().await {
            Ok(msg) => msg,
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "就绪检查，确认Prometheus服务是否就绪")]
    pub async fn prom_ready(&self) -> String {
        info!("执行就绪检查!");
        match self.searcher.prometheus.ready().await {
            Ok(msg) => msg,
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "重载Prometheus配置文件")]
    pub async fn prom_reload(&self) -> String {
        info!("重载Prometheus配置!");
        match self.searcher.prometheus.reload().await {
            Ok(msg) => msg,
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "优雅关闭Prometheus服务")]
    pub async fn prom_quit(&self) -> String {
        info!("关闭Prometheus服务!");
        match self.searcher.prometheus.quit().await {
            Ok(msg) => msg,
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "列出所有可用的 Prometheus 官方文档文件")]
    pub async fn docs_list(&self) -> String {
        info!("列出文档文件!");
        let loader = self.docs_loader.read().await;
        match loader.as_ref() {
            Some(docs) => {
                let files = docs.list_files();
                serde_json::json!({
                    "files": files,
                    "count": files.len()
                })
                .to_string()
            }
            None => "Error: Docs loader not initialized".to_string(),
        }
    }

    #[tool(description = "读取指定的 Prometheus 官方文档文件内容")]
    pub async fn docs_read(&self, Parameters(params): Parameters<DocsReadRequest>) -> String {
        info!("读取文档文件: {}", params.file);
        let loader = self.docs_loader.read().await;
        match loader.as_ref() {
            Some(docs) => match docs.read_file(&params.file) {
                Ok(content) => serde_json::json!({
                    "file": params.file,
                    "content": content
                })
                .to_string(),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Docs loader not initialized".to_string(),
        }
    }

    #[tool(description = "执行 Loki LogQL 查询")]
    pub async fn loki_query(&self, Parameters(params): Parameters<LokiQueryRequest>) -> String {
        info!(
            "执行 Loki 查询: {} ({} to {})",
            params.query, params.start, params.end
        );
        match self
            .searcher
            .loki
            .query(&params.query, &params.start, &params.end, params.limit)
            .await
        {
            Ok(result) => {
                serde_json::to_string(&result).unwrap_or_else(|_| "Failed to serialize".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "获取 Loki 的所有标签名")]
    pub async fn loki_labels(&self, Parameters(params): Parameters<LokiLabelsRequest>) -> String {
        info!("获取 Loki 标签列表");
        match self
            .searcher
            .loki
            .labels(params.start.as_deref(), params.end.as_deref())
            .await
        {
            Ok(labels) => {
                serde_json::to_string(&labels).unwrap_or_else(|_| "Failed to serialize".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "获取 Loki 指定标签的所有可能值")]
    pub async fn loki_label_values(
        &self,
        Parameters(params): Parameters<LokiLabelValuesRequest>,
    ) -> String {
        info!("获取 Loki 标签 {} 的值列表", params.label_name);
        match self
            .searcher
            .loki
            .label_values(
                &params.label_name,
                params.start.as_deref(),
                params.end.as_deref(),
            )
            .await
        {
            Ok(values) => {
                serde_json::to_string(&values).unwrap_or_else(|_| "Failed to serialize".to_string())
            }
            Err(e) => format!("Error: {}", e),
        }
    }

    #[tool(description = "在 Prometheus 官方文档中搜索关键词")]
    pub async fn docs_search(&self, Parameters(params): Parameters<DocsSearchRequest>) -> String {
        let limit = params.limit.unwrap_or(10);
        info!("搜索文档: query={}, limit={}", params.query, limit);
        let loader = self.docs_loader.read().await;
        match loader.as_ref() {
            Some(docs) => {
                let results = docs.search(&params.query, limit);
                serde_json::json!({
                    "query": params.query,
                    "matching_files": results,
                    "count": results.len()
                })
                .to_string()
            }
            None => "Error: Docs loader not initialized".to_string(),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Tools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_06_18,
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                ..Default::default()
            },
            instructions: Some(
                "Prometheus MCP Sserver providing metrics search for business!".into(),
            ),
            server_info: Implementation {
                name: "prometheus-mcp-server".into(),
                version: "0.1.0".into(),
                ..Default::default()
            },
        }
    }

    async fn ping(&self, _ctx: RequestContext<RoleServer>) -> Result<(), ErrorData> {
        info!("Received ping request");
        Ok(())
    }

    async fn on_initialized(&self, _ctx: NotificationContext<RoleServer>) {
        info!("Client initialized successfully");
    }
}
