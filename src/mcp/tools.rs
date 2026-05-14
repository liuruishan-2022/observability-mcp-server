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

// ========== Harbor 相关数据结构 ==========

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct HarborGetProjectRequest {
    #[schemars(description = "项目 ID 或项目名称")]
    pub project_id_or_name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct HarborCreateProjectRequest {
    #[schemars(description = "项目名称")]
    pub project_name: String,
    #[schemars(description = "是否为公开项目")]
    pub public: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct HarborDeleteProjectRequest {
    #[schemars(description = "项目 ID 或项目名称")]
    pub project_id_or_name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct HarborGetRepositoriesRequest {
    #[schemars(description = "项目 ID 或项目名称")]
    pub project_id_or_name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct HarborDeleteRepositoryRequest {
    #[schemars(description = "项目 ID 或项目名称")]
    pub project_id_or_name: String,
    #[schemars(description = "仓库名称")]
    pub repository_name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct HarborGetArtifactsRequest {
    #[schemars(description = "项目 ID 或项目名称")]
    pub project_id_or_name: String,
    #[schemars(description = "仓库名称")]
    pub repository_name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct HarborDeleteArtifactRequest {
    #[schemars(description = "项目 ID 或项目名称")]
    pub project_id_or_name: String,
    #[schemars(description = "仓库名称")]
    pub repository_name: String,
    #[schemars(description = "artifact digest")]
    pub digest: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct HarborGetHelmChartsRequest {
    #[schemars(description = "项目 ID 或项目名称")]
    pub project_id_or_name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct HarborGetHelmChartVersionsRequest {
    #[schemars(description = "项目 ID 或项目名称")]
    pub project_id_or_name: String,
    #[schemars(description = "Chart 名称")]
    pub chart_name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct HarborDeleteHelmChartVersionRequest {
    #[schemars(description = "项目 ID 或项目名称")]
    pub project_id_or_name: String,
    #[schemars(description = "Chart 名称")]
    pub chart_name: String,
    #[schemars(description = "Chart 版本")]
    pub version: String,
}

// ========== Nacos 相关数据结构 ==========

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NacosListNamespacesRequest {}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NacosListServicesRequest {
    #[schemars(description = "当前页码，默认为 1")]
    pub page_no: i32,
    #[schemars(description = "每页服务数量，默认为 100")]
    pub page_size: i32,
    #[schemars(description = "命名空间 ID，默认为 public")]
    pub namespace_id: Option<String>,
    #[schemars(description = "服务分组名称模式")]
    pub group_name_param: Option<String>,
    #[schemars(description = "服务名称模式")]
    pub service_name_param: Option<String>,
    #[schemars(description = "是否忽略空服务")]
    pub ignore_empty_service: Option<bool>,
    #[schemars(description = "是否包含实例信息")]
    pub with_instances: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NacosGetServiceRequest {
    #[schemars(description = "命名空间 ID，默认为 public")]
    pub namespace_id: Option<String>,
    #[schemars(description = "服务分组名称，默认为 DEFAULT_GROUP")]
    pub group_name: Option<String>,
    #[schemars(description = "服务名称")]
    pub service_name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NacosListInstancesRequest {
    #[schemars(description = "命名空间 ID，默认为 public")]
    pub namespace_id: Option<String>,
    #[schemars(description = "服务分组名称，默认为 DEFAULT_GROUP")]
    pub group_name: Option<String>,
    #[schemars(description = "服务名称")]
    pub service_name: String,
    #[schemars(description = "集群名称")]
    pub cluster_name: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NacosListServiceSubscribersRequest {
    #[schemars(description = "当前页码，默认为 1")]
    pub page_no: i32,
    #[schemars(description = "每页订阅者数量，默认为 100")]
    pub page_size: i32,
    #[schemars(description = "命名空间 ID，默认为 public")]
    pub namespace_id: Option<String>,
    #[schemars(description = "服务分组名称，默认为 DEFAULT_GROUP")]
    pub group_name: Option<String>,
    #[schemars(description = "服务名称")]
    pub service_name: String,
    #[schemars(description = "是否聚合整个集群")]
    pub aggregation: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NacosListConfigsRequest {
    #[schemars(description = "当前页码，默认为 1")]
    pub page_no: i32,
    #[schemars(description = "每页配置数量，默认为 100")]
    pub page_size: i32,
    #[schemars(description = "命名空间 ID，默认为 public")]
    pub namespace_id: Option<String>,
    #[schemars(description = "配置分组名称模式")]
    pub group_name: Option<String>,
    #[schemars(description = "配置 Data ID 模式")]
    pub data_id: Option<String>,
    #[schemars(description = "配置类型")]
    #[serde(rename = "type")]
    pub config_type: Option<String>,
    #[schemars(description = "配置标签")]
    pub config_tags: Option<String>,
    #[schemars(description = "应用名称")]
    pub app_name: Option<String>,
    #[schemars(description = "搜索方式：blur(模糊) 或 accurate(精确)")]
    pub search: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NacosGetConfigRequest {
    #[schemars(description = "命名空间 ID，默认为 public")]
    pub namespace_id: Option<String>,
    #[schemars(description = "配置分组名称")]
    pub group_name: String,
    #[schemars(description = "配置 Data ID")]
    pub data_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NacosListConfigHistoryRequest {
    #[schemars(description = "当前页码，默认为 1")]
    pub page_no: i32,
    #[schemars(description = "每页历史记录数量，默认为 100")]
    pub page_size: i32,
    #[schemars(description = "命名空间 ID，默认为 public")]
    pub namespace_id: Option<String>,
    #[schemars(description = "配置分组名称")]
    pub group_name: String,
    #[schemars(description = "配置 Data ID")]
    pub data_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NacosGetConfigHistoryRequest {
    #[schemars(description = "命名空间 ID，默认为 public")]
    pub namespace_id: Option<String>,
    #[schemars(description = "配置分组名称")]
    pub group_name: String,
    #[schemars(description = "配置 Data ID")]
    pub data_id: String,
    #[schemars(description = "历史记录 ID")]
    pub nid: Option<i64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NacosListConfigListenersRequest {
    #[schemars(description = "命名空间 ID，默认为 public")]
    pub namespace_id: Option<String>,
    #[schemars(description = "配置分组名称")]
    pub group_name: String,
    #[schemars(description = "配置 Data ID")]
    pub data_id: String,
    #[schemars(description = "是否聚合整个集群")]
    pub aggregation: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct NacosListListenedConfigsRequest {
    #[schemars(description = "命名空间 ID，默认为 public")]
    pub namespace_id: Option<String>,
    #[schemars(description = "客户端 IP")]
    pub ip: String,
    #[schemars(description = "是否聚合整个集群")]
    pub aggregation: Option<bool>,
}

// ========== Kafka 相关数据结构 ==========

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct KafkaCreateTopicRequest {
    #[schemars(description = "主题名称")]
    pub topic: String,
    #[schemars(description = "分区数量，默认为 1")]
    pub num_partitions: Option<i32>,
    #[schemars(description = "副本因子，默认为 1")]
    pub replication_factor: Option<i32>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct KafkaListTopicsRequest {}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct KafkaDeleteTopicRequest {
    #[schemars(description = "要删除的主题名称")]
    pub topic: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct KafkaDescribeTopicRequest {
    #[schemars(description = "要描述的主题名称")]
    pub topic: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct KafkaProduceMessageRequest {
    #[schemars(description = "主题名称")]
    pub topic: String,
    #[schemars(description = "消息键 (可选，如不提供将自动生成 UUID)")]
    pub key: Option<String>,
    #[schemars(description = "消息内容")]
    pub value: String,
    #[schemars(description = "消息头 (可选)，格式为 {\"key1\": \"value1\", \"key2\": \"value2\"}")]
    pub headers: Option<Vec<(String, String)>>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct KafkaConsumeMessagesRequest {
    #[schemars(description = "主题名称")]
    pub topic: String,
    #[schemars(description = "消费超时时间（秒），默认为 10")]
    pub timeout_seconds: Option<i32>,
}

// ========== Doris 相关数据结构 ==========

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetDatabasesRequest {}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetTablesRequest {
    #[schemars(description = "数据库名称")]
    pub database: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetTableSchemaRequest {
    #[schemars(description = "数据库名称")]
    pub database: String,
    #[schemars(description = "表名称")]
    pub table: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetTableMetadataRequest {
    #[schemars(description = "数据库名称")]
    pub database: String,
    #[schemars(description = "表名称")]
    pub table: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetFeStatusRequest {}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetBeStatusRequest {}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetQueryStatsRequest {}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetRoutineLoadsRequest {}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetLoadJobsRequest {}

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

    // ========== Harbor Tools ==========

    #[tool(description = "获取 Harbor 所有项目列表")]
    pub async fn harbor_get_projects(&self) -> String {
        info!("获取 Harbor 项目列表");
        match self.searcher.harbor() {
            Some(harbor) => match harbor.get_projects().await {
                Ok(projects) => serde_json::to_string(&projects)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Harbor client not configured. Please set HARBOR_URL, HARBOR_USERNAME, and HARBOR_PASSWORD environment variables.".to_string(),
        }
    }

    #[tool(description = "获取 Harbor 指定项目的信息")]
    pub async fn harbor_get_project(&self, Parameters(params): Parameters<HarborGetProjectRequest>) -> String {
        info!("获取 Harbor 项目: {}", params.project_id_or_name);
        match self.searcher.harbor() {
            Some(harbor) => match harbor.get_project(&params.project_id_or_name).await {
                Ok(project) => serde_json::to_string(&project)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Harbor client not configured".to_string(),
        }
    }

    #[tool(description = "创建 Harbor 项目")]
    pub async fn harbor_create_project(&self, Parameters(params): Parameters<HarborCreateProjectRequest>) -> String {
        info!("创建 Harbor 项目: {}", params.project_name);
        match self.searcher.harbor() {
            Some(harbor) => {
                use crate::searcher::harbor::CreateProjectRequest;
                let request = CreateProjectRequest {
                    project_name: params.project_name.clone(),
                    public: params.public,
                    metadata: None,
                };
                match harbor.create_project(&request).await {
                    Ok(project) => serde_json::to_string(&project)
                        .unwrap_or_else(|_| "Failed to serialize".to_string()),
                    Err(e) => format!("Error: {}", e),
                }
            },
            None => "Error: Harbor client not configured".to_string(),
        }
    }

    #[tool(description = "删除 Harbor 项目")]
    pub async fn harbor_delete_project(&self, Parameters(params): Parameters<HarborDeleteProjectRequest>) -> String {
        info!("删除 Harbor 项目: {}", params.project_id_or_name);
        match self.searcher.harbor() {
            Some(harbor) => match harbor.delete_project(&params.project_id_or_name).await {
                Ok(()) => serde_json::json!({
                    "status": "success",
                    "message": format!("Project '{}' deleted successfully", params.project_id_or_name)
                }).to_string(),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Harbor client not configured".to_string(),
        }
    }

    #[tool(description = "获取项目的仓库列表")]
    pub async fn harbor_get_repositories(&self, Parameters(params): Parameters<HarborGetRepositoriesRequest>) -> String {
        info!("获取 Harbor 仓库列表: project={}", params.project_id_or_name);
        match self.searcher.harbor() {
            Some(harbor) => match harbor.get_repositories(&params.project_id_or_name).await {
                Ok(repositories) => serde_json::to_string(&repositories)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Harbor client not configured".to_string(),
        }
    }

    #[tool(description = "删除仓库")]
    pub async fn harbor_delete_repository(&self, Parameters(params): Parameters<HarborDeleteRepositoryRequest>) -> String {
        info!("删除 Harbor 仓库: project={}, repo={}", params.project_id_or_name, params.repository_name);
        match self.searcher.harbor() {
            Some(harbor) => match harbor.delete_repository(&params.project_id_or_name, &params.repository_name).await {
                Ok(()) => serde_json::json!({
                    "status": "success",
                    "message": format!("Repository '{}/{}' deleted successfully", params.project_id_or_name, params.repository_name)
                }).to_string(),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Harbor client not configured".to_string(),
        }
    }

    #[tool(description = "获取仓库的 artifacts (镜像标签) 列表")]
    pub async fn harbor_get_artifacts(&self, Parameters(params): Parameters<HarborGetArtifactsRequest>) -> String {
        info!("获取 Harbor artifacts: project={}, repo={}", params.project_id_or_name, params.repository_name);
        match self.searcher.harbor() {
            Some(harbor) => match harbor.get_artifacts(&params.project_id_or_name, &params.repository_name).await {
                Ok(artifacts) => serde_json::to_string(&artifacts)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Harbor client not configured".to_string(),
        }
    }

    #[tool(description = "删除 artifact (镜像标签)")]
    pub async fn harbor_delete_artifact(&self, Parameters(params): Parameters<HarborDeleteArtifactRequest>) -> String {
        info!("删除 Harbor artifact: project={}, repo={}, digest={}",
            params.project_id_or_name, params.repository_name, params.digest);
        match self.searcher.harbor() {
            Some(harbor) => match harbor.delete_artifact(
                &params.project_id_or_name,
                &params.repository_name,
                &params.digest
            ).await {
                Ok(()) => serde_json::json!({
                    "status": "success",
                    "message": format!("Artifact '{}/{}@{}' deleted successfully",
                        params.project_id_or_name, params.repository_name, params.digest)
                }).to_string(),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Harbor client not configured".to_string(),
        }
    }

    #[tool(description = "获取项目的 Helm Charts 列表")]
    pub async fn harbor_get_helm_charts(&self, Parameters(params): Parameters<HarborGetHelmChartsRequest>) -> String {
        info!("获取 Harbor Helm Charts: project={}", params.project_id_or_name);
        match self.searcher.harbor() {
            Some(harbor) => match harbor.get_helm_charts(&params.project_id_or_name).await {
                Ok(charts) => serde_json::to_string(&charts)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Harbor client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Helm Chart 的版本列表")]
    pub async fn harbor_get_helm_chart_versions(&self, Parameters(params): Parameters<HarborGetHelmChartVersionsRequest>) -> String {
        info!("获取 Harbor Helm Chart 版本: project={}, chart={}",
            params.project_id_or_name, params.chart_name);
        match self.searcher.harbor() {
            Some(harbor) => match harbor.get_helm_chart_versions(
                &params.project_id_or_name,
                &params.chart_name
            ).await {
                Ok(versions) => serde_json::to_string(&versions)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Harbor client not configured".to_string(),
        }
    }

    #[tool(description = "删除 Helm Chart 版本")]
    pub async fn harbor_delete_helm_chart_version(&self, Parameters(params): Parameters<HarborDeleteHelmChartVersionRequest>) -> String {
        info!("删除 Harbor Helm Chart 版本: project={}, chart={}, version={}",
            params.project_id_or_name, params.chart_name, params.version);
        match self.searcher.harbor() {
            Some(harbor) => match harbor.delete_helm_chart_version(
                &params.project_id_or_name,
                &params.chart_name,
                &params.version
            ).await {
                Ok(()) => serde_json::json!({
                    "status": "success",
                    "message": format!("Helm Chart '{}/{}:{}' deleted successfully",
                        params.project_id_or_name, params.chart_name, params.version)
                }).to_string(),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Harbor client not configured".to_string(),
        }
    }

    // ========== Nacos Tools ==========

    #[tool(description = "获取 Nacos 所有命名空间列表")]
    pub async fn nacos_list_namespaces(&self, _params: Parameters<NacosListNamespacesRequest>) -> String {
        info!("获取 Nacos 命名空间列表");
        match self.searcher.nacos() {
            Some(nacos) => match nacos.list_namespaces().await {
                Ok(namespaces) => serde_json::to_string(&namespaces)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Nacos client not configured. Please set NACOS_URL environment variable.".to_string(),
        }
    }

    #[tool(description = "获取 Nacos 服务列表")]
    pub async fn nacos_list_services(&self, Parameters(params): Parameters<NacosListServicesRequest>) -> String {
        info!("获取 Nacos 服务列表: namespace={:?}", params.namespace_id);
        match self.searcher.nacos() {
            Some(nacos) => {
                use crate::searcher::nacos::ListServicesParams;
                let request = ListServicesParams {
                    page_no: params.page_no,
                    page_size: params.page_size,
                    namespace_id: params.namespace_id.clone(),
                    group_name_param: params.group_name_param.clone(),
                    service_name_param: params.service_name_param.clone(),
                    ignore_empty_service: params.ignore_empty_service,
                    with_instances: params.with_instances,
                };
                match nacos.list_services(&request).await {
                    Ok(services) => serde_json::to_string(&services)
                        .unwrap_or_else(|_| "Failed to serialize".to_string()),
                    Err(e) => format!("Error: {}", e),
                }
            },
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Nacos 指定服务的详情")]
    pub async fn nacos_get_service(&self, Parameters(params): Parameters<NacosGetServiceRequest>) -> String {
        info!("获取 Nacos 服务详情: service={}", params.service_name);
        match self.searcher.nacos() {
            Some(nacos) => {
                use crate::searcher::nacos::GetServiceParams;
                let request = GetServiceParams {
                    namespace_id: params.namespace_id.clone(),
                    group_name: params.group_name.clone(),
                    service_name: params.service_name.clone(),
                };
                match nacos.get_service(&request).await {
                    Ok(service) => serde_json::to_string(&service)
                        .unwrap_or_else(|_| "Failed to serialize".to_string()),
                    Err(e) => format!("Error: {}", e),
                }
            },
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Nacos 服务实例列表")]
    pub async fn nacos_list_instances(&self, Parameters(params): Parameters<NacosListInstancesRequest>) -> String {
        info!("获取 Nacos 服务实例: service={}", params.service_name);
        match self.searcher.nacos() {
            Some(nacos) => {
                use crate::searcher::nacos::ListInstancesParams;
                let request = ListInstancesParams {
                    namespace_id: params.namespace_id.clone(),
                    group_name: params.group_name.clone(),
                    service_name: params.service_name.clone(),
                    cluster_name: params.cluster_name.clone(),
                };
                match nacos.list_instances(&request).await {
                    Ok(instances) => serde_json::to_string(&instances)
                        .unwrap_or_else(|_| "Failed to serialize".to_string()),
                    Err(e) => format!("Error: {}", e),
                }
            },
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Nacos 服务订阅者列表")]
    pub async fn nacos_list_service_subscribers(&self, Parameters(params): Parameters<NacosListServiceSubscribersRequest>) -> String {
        info!("获取 Nacos 服务订阅者: service={}", params.service_name);
        match self.searcher.nacos() {
            Some(nacos) => {
                use crate::searcher::nacos::ListServiceSubscribersParams;
                let request = ListServiceSubscribersParams {
                    page_no: params.page_no,
                    page_size: params.page_size,
                    namespace_id: params.namespace_id.clone(),
                    group_name: params.group_name.clone(),
                    service_name: params.service_name.clone(),
                    aggregation: params.aggregation,
                };
                match nacos.list_service_subscribers(&request).await {
                    Ok(subscribers) => serde_json::to_string(&subscribers)
                        .unwrap_or_else(|_| "Failed to serialize".to_string()),
                    Err(e) => format!("Error: {}", e),
                }
            },
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Nacos 配置列表")]
    pub async fn nacos_list_configs(&self, Parameters(params): Parameters<NacosListConfigsRequest>) -> String {
        info!("获取 Nacos 配置列表");
        match self.searcher.nacos() {
            Some(nacos) => {
                use crate::searcher::nacos::ListConfigsParams;
                let request = ListConfigsParams {
                    page_no: params.page_no,
                    page_size: params.page_size,
                    namespace_id: params.namespace_id.clone(),
                    group_name: params.group_name.clone(),
                    data_id: params.data_id.clone(),
                    config_type: params.config_type.clone(),
                    config_tags: params.config_tags.clone(),
                    app_name: params.app_name.clone(),
                    search: params.search.clone(),
                };
                match nacos.list_configs(&request).await {
                    Ok(configs) => serde_json::to_string(&configs)
                        .unwrap_or_else(|_| "Failed to serialize".to_string()),
                    Err(e) => format!("Error: {}", e),
                }
            },
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Nacos 配置详情")]
    pub async fn nacos_get_config(&self, Parameters(params): Parameters<NacosGetConfigRequest>) -> String {
        info!("获取 Nacos 配置详情: dataId={}, group={}", params.data_id, params.group_name);
        match self.searcher.nacos() {
            Some(nacos) => {
                use crate::searcher::nacos::GetConfigParams;
                let request = GetConfigParams {
                    namespace_id: params.namespace_id.clone(),
                    group_name: params.group_name.clone(),
                    data_id: params.data_id.clone(),
                };
                match nacos.get_config(&request).await {
                    Ok(config) => serde_json::to_string(&config)
                        .unwrap_or_else(|_| "Failed to serialize".to_string()),
                    Err(e) => format!("Error: {}", e),
                }
            },
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Nacos 配置历史列表")]
    pub async fn nacos_list_config_history(&self, Parameters(params): Parameters<NacosListConfigHistoryRequest>) -> String {
        info!("获取 Nacos 配置历史: dataId={}, group={}", params.data_id, params.group_name);
        match self.searcher.nacos() {
            Some(nacos) => {
                use crate::searcher::nacos::ListConfigHistoryParams;
                let request = ListConfigHistoryParams {
                    page_no: params.page_no,
                    page_size: params.page_size,
                    namespace_id: params.namespace_id.clone(),
                    group_name: params.group_name.clone(),
                    data_id: params.data_id.clone(),
                };
                match nacos.list_config_history(&request).await {
                    Ok(history) => serde_json::to_string(&history)
                        .unwrap_or_else(|_| "Failed to serialize".to_string()),
                    Err(e) => format!("Error: {}", e),
                }
            },
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Nacos 配置历史详情")]
    pub async fn nacos_get_config_history(&self, Parameters(params): Parameters<NacosGetConfigHistoryRequest>) -> String {
        info!("获取 Nacos 配置历史详情: dataId={}, group={}, nid={:?}", params.data_id, params.group_name, params.nid);
        match self.searcher.nacos() {
            Some(nacos) => {
                use crate::searcher::nacos::GetConfigHistoryParams;
                let request = GetConfigHistoryParams {
                    namespace_id: params.namespace_id.clone(),
                    group_name: params.group_name.clone(),
                    data_id: params.data_id.clone(),
                    nid: params.nid,
                };
                match nacos.get_config_history(&request).await {
                    Ok(detail) => serde_json::to_string(&detail)
                        .unwrap_or_else(|_| "Failed to serialize".to_string()),
                    Err(e) => format!("Error: {}", e),
                }
            },
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Nacos 配置监听器列表")]
    pub async fn nacos_list_config_listeners(&self, Parameters(params): Parameters<NacosListConfigListenersRequest>) -> String {
        info!("获取 Nacos 配置监听器: dataId={}, group={}", params.data_id, params.group_name);
        match self.searcher.nacos() {
            Some(nacos) => {
                use crate::searcher::nacos::ListConfigListenersParams;
                let request = ListConfigListenersParams {
                    namespace_id: params.namespace_id.clone(),
                    group_name: params.group_name.clone(),
                    data_id: params.data_id.clone(),
                    aggregation: params.aggregation,
                };
                match nacos.list_config_listeners(&request).await {
                    Ok(listeners) => serde_json::to_string(&listeners)
                        .unwrap_or_else(|_| "Failed to serialize".to_string()),
                    Err(e) => format!("Error: {}", e),
                }
            },
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    #[tool(description = "获取客户端监听的 Nacos 配置列表")]
    pub async fn nacos_list_listened_configs(&self, Parameters(params): Parameters<NacosListListenedConfigsRequest>) -> String {
        info!("获取客户端监听的 Nacos 配置: ip={}", params.ip);
        match self.searcher.nacos() {
            Some(nacos) => {
                use crate::searcher::nacos::ListListenedConfigsParams;
                let request = ListListenedConfigsParams {
                    namespace_id: params.namespace_id.clone(),
                    ip: params.ip.clone(),
                    aggregation: params.aggregation,
                };
                match nacos.list_listened_configs(&request).await {
                    Ok(configs) => serde_json::to_string(&configs)
                        .unwrap_or_else(|_| "Failed to serialize".to_string()),
                    Err(e) => format!("Error: {}", e),
                }
            },
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    // ========== Kafka Tools ==========

    #[tool(description = "创建 Kafka 主题")]
    pub async fn kafka_create_topic(&self, Parameters(params): Parameters<KafkaCreateTopicRequest>) -> String {
        info!("创建 Kafka 主题: {}", params.topic);
        match self.searcher.kafka() {
            Some(kafka) => {
                let num_partitions = params.num_partitions.unwrap_or(1);
                let replication_factor = params.replication_factor.unwrap_or(1);
                match kafka.create_topic(&params.topic, num_partitions, replication_factor).await {
                    Ok(result) => result,
                    Err(e) => format!("Error: {}", e),
                }
            },
            None => "Error: Kafka client not configured. Please set KAFKA_BOOTSTRAP_SERVERS environment variable.".to_string(),
        }
    }

    #[tool(description = "列出 Kafka 所有主题")]
    pub async fn kafka_list_topics(&self, _params: Parameters<KafkaListTopicsRequest>) -> String {
        info!("列出 Kafka 所有主题");
        match self.searcher.kafka() {
            Some(kafka) => match kafka.list_topics().await {
                Ok(topics) => serde_json::to_string(&topics)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Kafka client not configured".to_string(),
        }
    }

    #[tool(description = "删除 Kafka 主题")]
    pub async fn kafka_delete_topic(&self, Parameters(params): Parameters<KafkaDeleteTopicRequest>) -> String {
        info!("删除 Kafka 主题: {}", params.topic);
        match self.searcher.kafka() {
            Some(kafka) => match kafka.delete_topic(&params.topic).await {
                Ok(result) => result,
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Kafka client not configured".to_string(),
        }
    }

    #[tool(description = "描述 Kafka 主题详情")]
    pub async fn kafka_describe_topic(&self, Parameters(params): Parameters<KafkaDescribeTopicRequest>) -> String {
        info!("描述 Kafka 主题: {}", params.topic);
        match self.searcher.kafka() {
            Some(kafka) => match kafka.describe_topic(&params.topic).await {
                Ok(metadata) => serde_json::to_string(&metadata)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Kafka client not configured".to_string(),
        }
    }

    #[tool(description = "生产消息到 Kafka 主题")]
    pub async fn kafka_produce_message(&self, Parameters(params): Parameters<KafkaProduceMessageRequest>) -> String {
        info!("生产消息到 Kafka 主题: {}", params.topic);
        match self.searcher.kafka() {
            Some(kafka) => match kafka.produce_message(
                &params.topic,
                params.key.clone(),
                &params.value,
                params.headers.clone(),
            ).await {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Kafka client not configured".to_string(),
        }
    }

    #[tool(description = "从 Kafka 主题消费消息")]
    pub async fn kafka_consume_messages(&self, Parameters(params): Parameters<KafkaConsumeMessagesRequest>) -> String {
        info!("从 Kafka 主题消费���息: {}", params.topic);
        match self.searcher.kafka() {
            Some(kafka) => match kafka.consume_messages(&params.topic, params.timeout_seconds).await {
                Ok(messages) => serde_json::to_string(&messages)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Kafka client not configured".to_string(),
        }
    }

    // ========== Doris Tools ==========

    #[tool(description = "获取 Doris 所有数据库列表")]
    pub async fn doris_get_databases(&self, _params: Parameters<DorisGetDatabasesRequest>) -> String {
        info!("获取 Doris 数据库列表");
        match self.searcher.doris() {
            Some(doris) => match doris.get_databases().await {
                Ok(dbs) => serde_json::to_string(&dbs)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured. Please set DORIS_URL environment variable.".to_string(),
        }
    }

    #[tool(description = "获取指定数据库的表列表")]
    pub async fn doris_get_tables(&self, Parameters(params): Parameters<DorisGetTablesRequest>) -> String {
        info!("获取 Doris 表列表: database={}", params.database);
        match self.searcher.doris() {
            Some(doris) => match doris.get_tables(&params.database).await {
                Ok(tables) => serde_json::to_string(&tables)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取表结构详情")]
    pub async fn doris_get_table_schema(&self, Parameters(params): Parameters<DorisGetTableSchemaRequest>) -> String {
        info!("获取 Doris 表结构: {}.{}", params.database, params.table);
        match self.searcher.doris() {
            Some(doris) => match doris.get_table_schema(&params.database, &params.table).await {
                Ok(schema) => serde_json::to_string(&schema)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取表元数据（大小、行数等）")]
    pub async fn doris_get_table_metadata(&self, Parameters(params): Parameters<DorisGetTableMetadataRequest>) -> String {
        info!("获取 Doris 表元数据: {}.{}", params.database, params.table);
        match self.searcher.doris() {
            Some(doris) => match doris.get_table_metadata(&params.database, &params.table).await {
                Ok(metadata) => serde_json::to_string(&metadata)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Doris FE (Frontend) 节点状态")]
    pub async fn doris_get_fe_status(&self, _params: Parameters<DorisGetFeStatusRequest>) -> String {
        info!("获取 Doris FE 节点状态");
        match self.searcher.doris() {
            Some(doris) => match doris.get_fe_status().await {
                Ok(status) => serde_json::to_string(&status)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured. Please set DORIS_HTTP_URL environment variable.".to_string(),
        }
    }

    #[tool(description = "获取 Doris BE (Backend) 节点状态")]
    pub async fn doris_get_be_status(&self, _params: Parameters<DorisGetBeStatusRequest>) -> String {
        info!("获取 Doris BE 节点状态");
        match self.searcher.doris() {
            Some(doris) => match doris.get_be_status().await {
                Ok(status) => serde_json::to_string(&status)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured. Please set DORIS_HTTP_URL environment variable.".to_string(),
        }
    }

    #[tool(description = "获取 Doris 查询统计信息")]
    pub async fn doris_get_query_stats(&self, _params: Parameters<DorisGetQueryStatsRequest>) -> String {
        info!("获取 Doris 查询统计");
        match self.searcher.doris() {
            Some(doris) => match doris.get_query_stats().await {
                Ok(stats) => serde_json::to_string(&stats)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Doris Routine Load 任务列表")]
    pub async fn doris_get_routine_loads(&self, _params: Parameters<DorisGetRoutineLoadsRequest>) -> String {
        info!("获取 Doris Routine Load 任务列表");
        match self.searcher.doris() {
            Some(doris) => match doris.get_routine_loads().await {
                Ok(jobs) => serde_json::to_string(&jobs)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Doris Load 任务列表")]
    pub async fn doris_get_load_jobs(&self, _params: Parameters<DorisGetLoadJobsRequest>) -> String {
        info!("获取 Doris Load 任务列表");
        match self.searcher.doris() {
            Some(doris) => match doris.get_load_jobs().await {
                Ok(jobs) => serde_json::to_string(&jobs)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Tools {
    fn get_info(&self) -> ServerInfo {
        let mut capabilities = ServerCapabilities::default();
        capabilities.tools = Some(ToolsCapability {
            list_changed: Some(false),
        });

        ServerInfo::new(capabilities)
            .with_protocol_version(ProtocolVersion::V_2025_06_18)
            .with_instructions(
                "Observability MCP Server providing Prometheus, Loki metrics search, Harbor container registry management, Nacos service discovery and configuration management, Kafka messaging, and Doris database operations!",
            )
            .with_server_info(Implementation::new("observability-mcp-server", "0.4.0"))
    }

    async fn ping(&self, _ctx: RequestContext<RoleServer>) -> Result<(), ErrorData> {
        info!("Received ping request");
        Ok(())
    }

    async fn on_initialized(&self, _ctx: NotificationContext<RoleServer>) {
        info!("Client initialized successfully");
    }
}
