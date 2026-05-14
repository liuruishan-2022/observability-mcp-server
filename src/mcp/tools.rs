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
pub struct DorisGetDatabasesRequest {
    #[schemars(description = "Catalog 名称，可选")]
    pub catalog_name: Option<String>,
}

// ========== Kubernetes 相关数据结构 ==========

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct KubeListPodsRequest {
    #[schemars(description = "命名空间，可选")]
    pub namespace: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct KubeGetPodRequest {
    #[schemars(description = "Pod 名称")]
    pub name: String,
    #[schemars(description = "命名空间，可选")]
    pub namespace: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct KubeDeletePodRequest {
    #[schemars(description = "Pod 名称")]
    pub name: String,
    #[schemars(description = "命名空间，可选")]
    pub namespace: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct KubeListNamespacesRequest {}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct KubeListEventsRequest {
    #[schemars(description = "命名空间，可选")]
    pub namespace: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct KubeListContextsRequest {}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct KubeGetConfigRequest {
    #[schemars(description = "是否精简输出，仅显示当前 context 相关信息")]
    pub minified: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetTablesRequest {
    #[schemars(description = "数据库名称，可选，默认使用 DORIS_DB")]
    #[serde(alias = "database")]
    pub db_name: Option<String>,
    #[schemars(description = "Catalog 名称，可选")]
    pub catalog_name: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetTableSchemaRequest {
    #[schemars(description = "数据库名称，可选，默认使用 DORIS_DB")]
    #[serde(alias = "database")]
    pub db_name: Option<String>,
    #[schemars(description = "表名称")]
    #[serde(alias = "table")]
    pub table_name: String,
    #[schemars(description = "Catalog 名称，可选")]
    pub catalog_name: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetTableMetadataRequest {
    #[schemars(description = "数据库名称，可选，默认使用 DORIS_DB")]
    #[serde(alias = "database")]
    pub db_name: Option<String>,
    #[schemars(description = "表名称")]
    #[serde(alias = "table")]
    pub table_name: String,
    #[schemars(description = "Catalog 名称，可选")]
    pub catalog_name: Option<String>,
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

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisExecQueryRequest {
    #[schemars(description = "SQL 语句")]
    pub sql: String,
    #[schemars(description = "数据库名称，可选，默认使用 DORIS_DB")]
    pub db_name: Option<String>,
    #[schemars(description = "Catalog 名称，可选")]
    pub catalog_name: Option<String>,
    #[schemars(description = "最大返回行数，默认 100")]
    pub max_rows: Option<usize>,
    #[schemars(description = "查询超时时间（秒），默认 30")]
    pub timeout: Option<u64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetTableCommentRequest {
    #[schemars(description = "数据库名称，可选，默认使用 DORIS_DB")]
    #[serde(alias = "database")]
    pub db_name: Option<String>,
    #[schemars(description = "表名称")]
    #[serde(alias = "table")]
    pub table_name: String,
    #[schemars(description = "Catalog 名称，可选")]
    pub catalog_name: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetTableIndexesRequest {
    #[schemars(description = "数据库名称，可选，默认使用 DORIS_DB")]
    #[serde(alias = "database")]
    pub db_name: Option<String>,
    #[schemars(description = "表名称")]
    #[serde(alias = "table")]
    pub table_name: String,
    #[schemars(description = "Catalog 名称，可选")]
    pub catalog_name: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetSqlExplainRequest {
    #[schemars(description = "SQL 语句")]
    pub sql: String,
    #[schemars(description = "是否显示详细信息")]
    pub verbose: Option<bool>,
    #[schemars(description = "数据库名称，可选，默认使用 DORIS_DB")]
    pub db_name: Option<String>,
    #[schemars(description = "Catalog 名称，可选")]
    pub catalog_name: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetCatalogListRequest {
    #[schemars(description = "占位参数，可选，仅为兼容官方 MCP 接口")]
    pub random_string: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetRecentAuditLogsRequest {
    #[schemars(description = "查询最近多少天的审计日志，默认 7")]
    pub days: Option<u32>,
    #[schemars(description = "最大返回条数，默认 100")]
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetSqlProfileRequest {
    #[schemars(description = "SQL 语句")]
    pub sql: String,
    #[schemars(description = "数据库名称，可选，默认使用 DORIS_DB")]
    pub db_name: Option<String>,
    #[schemars(description = "Catalog 名称，可选")]
    pub catalog_name: Option<String>,
    #[schemars(description = "查询超时时间（秒），默认 30")]
    pub timeout: Option<u64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetTableDataSizeRequest {
    #[schemars(description = "数据库名称，可选；不传则查询所有数据库")]
    #[serde(alias = "database")]
    pub db_name: Option<String>,
    #[schemars(description = "表名称，可选；不传则查询数据库下所有表")]
    #[serde(alias = "table")]
    pub table_name: Option<String>,
    #[schemars(description = "是否按单副本统计数据大小，默认 false")]
    pub single_replica: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetMemoryStatsRequest {
    #[schemars(description = "返回数据类型：realtime、historical 或 both，默认 realtime")]
    pub data_type: Option<String>,
    #[schemars(description = "实时内存统计的 tracker 类型，默认 overview")]
    pub tracker_type: Option<String>,
    #[schemars(description = "历史内存统计的 tracker 名称列表，可选")]
    pub tracker_names: Option<Vec<String>>,
    #[schemars(description = "历史内存统计时间范围，默认 1h")]
    pub time_range: Option<String>,
    #[schemars(description = "是否包含详细信息，默认 true")]
    pub include_details: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetMonitoringMetricsRequest {
    #[schemars(description = "返回内容类型：definitions、data 或 both，默认 data")]
    pub content_type: Option<String>,
    #[schemars(description = "节点角色：fe、be 或 all，默认 all")]
    pub role: Option<String>,
    #[schemars(description = "监控类型：process、jvm、machine 或 all，默认 all")]
    pub monitor_type: Option<String>,
    #[schemars(description = "优先级：core、p0 或 all，默认 core")]
    pub priority: Option<String>,
    #[schemars(description = "是否包含原始详细指标，默认 false")]
    pub include_raw_metrics: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetTableBasicInfoRequest {
    #[schemars(description = "表名称")]
    #[serde(alias = "table")]
    pub table_name: String,
    #[schemars(description = "Catalog 名称，可选")]
    pub catalog_name: Option<String>,
    #[schemars(description = "数据库名称，可选，默认使用 DORIS_DB")]
    #[serde(alias = "database")]
    pub db_name: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisAnalyzeColumnsRequest {
    #[schemars(description = "表名称")]
    #[serde(alias = "table")]
    pub table_name: String,
    #[schemars(description = "要分析的列名列表")]
    pub columns: Vec<String>,
    #[schemars(description = "分析类型：completeness、distribution 或 both")]
    pub analysis_types: Option<Vec<String>>,
    #[schemars(description = "最大采样行数，默认 100000")]
    pub sample_size: Option<usize>,
    #[schemars(description = "Catalog 名称，可选")]
    pub catalog_name: Option<String>,
    #[schemars(description = "数据库名称，可选，默认使用 DORIS_DB")]
    #[serde(alias = "database")]
    pub db_name: Option<String>,
    #[schemars(description = "是否返回更详细的数据，默认 false")]
    pub detailed_response: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisAnalyzeTableStorageRequest {
    #[schemars(description = "表名称")]
    #[serde(alias = "table")]
    pub table_name: String,
    #[schemars(description = "Catalog 名称，可选")]
    pub catalog_name: Option<String>,
    #[schemars(description = "数据库名称，可选，默认使用 DORIS_DB")]
    #[serde(alias = "database")]
    pub db_name: Option<String>,
    #[schemars(description = "是否返回更详细的数据，默认 false")]
    pub detailed_response: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisTraceColumnLineageRequest {
    #[schemars(description = "目标列列表，格式为 table.column 或 db.table.column")]
    pub target_columns: Vec<String>,
    #[schemars(description = "分析深度，默认 3")]
    pub analysis_depth: Option<u32>,
    #[schemars(description = "是否包含转换规则，默认 true")]
    pub include_transformations: Option<bool>,
    #[schemars(description = "Catalog 名称，可选")]
    pub catalog_name: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisMonitorDataFreshnessRequest {
    #[schemars(description = "待监控的表名列表；不传则分析当前库全部表")]
    pub table_names: Option<Vec<String>>,
    #[schemars(description = "新鲜度阈值（小时），默认 24")]
    pub freshness_threshold_hours: Option<u64>,
    #[schemars(description = "是否包含更新模式分析，默认 true")]
    pub include_update_patterns: Option<bool>,
    #[schemars(description = "Catalog 名称，可选")]
    pub catalog_name: Option<String>,
    #[schemars(description = "数据库名称，可选，默认使用 DORIS_DB")]
    #[serde(alias = "database")]
    pub db_name: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisAnalyzeDataAccessPatternsRequest {
    #[schemars(description = "分析最近多少天，默认 7")]
    pub days: Option<u32>,
    #[schemars(description = "是否包含系统用户，默认 false")]
    pub include_system_users: Option<bool>,
    #[schemars(description = "用户纳入分析的最小查询次数阈值，默认 5")]
    pub min_query_threshold: Option<u64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisAnalyzeDataFlowDependenciesRequest {
    #[schemars(description = "目标表名；不传则分析全部表")]
    #[serde(alias = "table")]
    pub target_table: Option<String>,
    #[schemars(description = "分析深度，默认 3")]
    pub analysis_depth: Option<u32>,
    #[schemars(description = "是否包含视图关系，默认 true")]
    pub include_views: Option<bool>,
    #[schemars(description = "Catalog 名称，可选")]
    pub catalog_name: Option<String>,
    #[schemars(description = "数据库名称，可选，默认使用 DORIS_DB")]
    #[serde(alias = "database")]
    pub db_name: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisAnalyzeSlowQueriesTopNRequest {
    #[schemars(description = "分析最近多少天，默认 7")]
    pub days: Option<u32>,
    #[schemars(description = "返回最慢查询 Top N，默认 20")]
    pub top_n: Option<usize>,
    #[schemars(description = "最小执行耗时阈值（毫秒），默认 1000")]
    pub min_execution_time_ms: Option<u64>,
    #[schemars(description = "是否包含 SQL 模式归类，默认 true")]
    pub include_patterns: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisAnalyzeResourceGrowthCurvesRequest {
    #[schemars(description = "分析最近多少天，默认 30")]
    pub days: Option<u32>,
    #[schemars(description = "资源类型列表，可选值包含 storage、query_volume、user_activity")]
    pub resource_types: Option<Vec<String>>,
    #[schemars(description = "是否包含预测结果，默认 false")]
    pub include_predictions: Option<bool>,
    #[schemars(description = "是否返回更详细的每日明细，默认 false")]
    pub detailed_response: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisExecAdbcQueryRequest {
    #[schemars(description = "SQL 语句")]
    pub sql: String,
    #[schemars(description = "最大返回行数；Rust 兼容实现会按当前系统上限裁剪")]
    pub max_rows: Option<usize>,
    #[schemars(description = "查询超时时间（秒），默认 60")]
    pub timeout: Option<u64>,
    #[schemars(description = "期望返回格式：arrow、pandas 或 dict")]
    pub return_format: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct DorisGetAdbcConnectionInfoRequest {}

// ========== Jira 相关数据结构 ==========

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetIssueRequest {
    #[schemars(description = "Jira issue key，例如 PROJ-123")]
    pub issue_key: String,
    #[schemars(description = "返回字段列表，例如 [\"summary\", \"status\", \"assignee\"]")]
    pub fields: Option<Vec<String>>,
    #[schemars(description = "扩展字段，例如 renderedFields,transitions,changelog")]
    pub expand: Option<String>,
    #[schemars(description = "返回评论的最大数量")]
    pub comment_limit: Option<usize>,
    #[schemars(description = "Issue properties 列表")]
    pub properties: Option<Vec<String>>,
    #[schemars(description = "是否更新浏览历史")]
    pub update_history: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraSearchRequest {
    #[schemars(description = "JQL 查询语句")]
    pub jql: String,
    #[schemars(description = "返回字段列表")]
    pub fields: Option<Vec<String>>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
    #[schemars(description = "起始偏移")]
    pub start_at: Option<usize>,
    #[schemars(description = "项目过滤器，逗号分隔，例如 DEV,OPS")]
    pub projects_filter: Option<String>,
    #[schemars(description = "扩展字段")]
    pub expand: Option<String>,
    #[schemars(description = "分页 token，主要用于 Cloud")]
    pub page_token: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraSearchFieldsRequest {
    #[schemars(description = "字段搜索关键字")]
    pub keyword: Option<String>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetFieldOptionsRequest {
    #[schemars(description = "字段 ID，例如 customfield_10001")]
    pub field_id: String,
    #[schemars(description = "字段上下文 ID，Cloud 可选")]
    pub context_id: Option<String>,
    #[schemars(description = "项目 key，Server/DC 获取字段选项时需要")]
    pub project_key: Option<String>,
    #[schemars(description = "Issue type，Server/DC 获取字段选项时需要")]
    pub issue_type: Option<String>,
    #[schemars(description = "按选项值模糊过滤")]
    pub contains: Option<String>,
    #[schemars(description = "最大返回数量")]
    pub return_limit: Option<usize>,
    #[schemars(description = "是否仅返回 value 字符串")]
    pub values_only: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetProjectIssuesRequest {
    #[schemars(description = "项目 key")]
    pub project_key: String,
    #[schemars(description = "返回字段列表")]
    pub fields: Option<Vec<String>>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
    #[schemars(description = "起始偏移")]
    pub start_at: Option<usize>,
    #[schemars(description = "扩展字段")]
    pub expand: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetTransitionsRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetWorklogRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetProjectVersionsRequest {
    #[schemars(description = "项目 key")]
    pub project_key: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetProjectComponentsRequest {
    #[schemars(description = "项目 key")]
    pub project_key: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetAllProjectsRequest {
    #[schemars(description = "是否包含归档项目")]
    pub include_archived: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetUserProfileRequest {
    #[schemars(description = "用户标识，例如邮箱、display name、accountId")]
    pub user_identifier: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetIssueWatchersRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraAddWatcherRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
    #[schemars(description = "用户标识，Cloud 推荐 accountId，Server/DC 推荐用户名")]
    pub user_identifier: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraRemoveWatcherRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
    #[schemars(description = "用户名，Server/DC 推荐使用")]
    pub username: Option<String>,
    #[schemars(description = "accountId，Cloud 推荐使用")]
    pub account_id: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraCreateIssueRequest {
    #[schemars(description = "项目 key")]
    pub project_key: String,
    #[schemars(description = "标题")]
    pub summary: String,
    #[schemars(description = "Issue 类型，例如 Task、Bug、Story")]
    pub issue_type: String,
    #[schemars(description = "指派人")]
    pub assignee: Option<String>,
    #[schemars(description = "Issue 描述")]
    pub description: Option<String>,
    #[schemars(description = "组件列表")]
    pub components: Option<Vec<String>>,
    #[schemars(description = "其他字段，必须是 JSON 对象")]
    pub additional_fields: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraUpdateIssueRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
    #[schemars(description = "待更新字段，必须是 JSON 对象")]
    pub fields: Option<serde_json::Value>,
    #[schemars(description = "附加更新字段，必须是 JSON 对象")]
    pub additional_fields: Option<serde_json::Value>,
    #[schemars(description = "组件列表")]
    pub components: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraDeleteIssueRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraAddCommentRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
    #[schemars(description = "评论内容")]
    pub comment: String,
    #[schemars(description = "评论可见性对象，例如 {\"type\":\"group\",\"value\":\"jira-users\"}")]
    pub visibility: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraAddWorklogRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
    #[schemars(description = "耗时，例如 1h30m、2h、45m")]
    pub time_spent: String,
    #[schemars(description = "Worklog 评论")]
    pub comment: Option<String>,
    #[schemars(description = "开始时间，ISO8601 格式")]
    pub started: Option<String>,
    #[schemars(description = "新的 original estimate")]
    pub original_estimate: Option<String>,
    #[schemars(description = "新的 remaining estimate")]
    pub remaining_estimate: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraTransitionIssueRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
    #[schemars(description = "Transition ID")]
    pub transition_id: String,
    #[schemars(description = "过渡时附带更新的字段，必须是 JSON 对象")]
    pub fields: Option<serde_json::Value>,
    #[schemars(description = "过渡时附带评论")]
    pub comment: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetLinkTypesRequest {}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraDownloadAttachmentsRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetIssueImagesRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetAgileBoardsRequest {
    #[schemars(description = "Board 名称过滤")]
    pub board_name: Option<String>,
    #[schemars(description = "项目 key 过滤")]
    pub project_key: Option<String>,
    #[schemars(description = "Board 类型过滤，如 scrum/kanban")]
    pub board_type: Option<String>,
    #[schemars(description = "分页起始位置")]
    pub start_at: Option<usize>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetBoardIssuesRequest {
    #[schemars(description = "Board ID")]
    pub board_id: String,
    #[schemars(description = "JQL 过滤条件")]
    pub jql: String,
    #[schemars(description = "返回字段列表")]
    pub fields: Option<Vec<String>>,
    #[schemars(description = "分页起始位置")]
    pub start_at: Option<usize>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
    #[schemars(description = "expand 参数")]
    pub expand: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetSprintsFromBoardRequest {
    #[schemars(description = "Board ID")]
    pub board_id: String,
    #[schemars(description = "Sprint 状态过滤")]
    pub state: Option<String>,
    #[schemars(description = "分页起始位置")]
    pub start_at: Option<usize>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetSprintIssuesRequest {
    #[schemars(description = "Sprint ID")]
    pub sprint_id: String,
    #[schemars(description = "返回字段列表")]
    pub fields: Option<Vec<String>>,
    #[schemars(description = "分页起始位置")]
    pub start_at: Option<usize>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraBatchCreateIssuesRequest {
    #[schemars(description = "批量创建 issue 的参数对象列表")]
    pub issues: Vec<serde_json::Value>,
    #[schemars(description = "是否仅校验，不真正创建")]
    pub validate_only: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraBatchGetChangelogsRequest {
    #[schemars(description = "Issue ID 或 key 列表")]
    pub issue_ids_or_keys: Vec<String>,
    #[schemars(description = "字段 ID 过滤列表")]
    pub fields: Option<Vec<String>>,
    #[schemars(description = "每个 issue 返回的最大 changelog 数量")]
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraEditCommentRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
    #[schemars(description = "评论 ID")]
    pub comment_id: String,
    #[schemars(description = "新的评论内容")]
    pub body: String,
    #[schemars(description = "评论可见性对象")]
    pub visibility: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraLinkToEpicRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
    #[schemars(description = "Epic issue key")]
    pub epic_key: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraCreateIssueLinkRequest {
    #[schemars(description = "Link 类型名称")]
    pub link_type: String,
    #[schemars(description = "Inward issue key")]
    pub inward_issue_key: String,
    #[schemars(description = "Outward issue key")]
    pub outward_issue_key: String,
    #[schemars(description = "可选评论")]
    pub comment: Option<String>,
    #[schemars(description = "评论可见性对象")]
    pub comment_visibility: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraCreateRemoteIssueLinkRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
    #[schemars(description = "远程链接 URL")]
    pub url: String,
    #[schemars(description = "远程链接标题")]
    pub title: String,
    #[schemars(description = "远程链接摘要")]
    pub summary: Option<String>,
    #[schemars(description = "关系描述")]
    pub relationship: Option<String>,
    #[schemars(description = "图标 URL")]
    pub icon_url: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraRemoveIssueLinkRequest {
    #[schemars(description = "Issue link ID")]
    pub link_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraCreateSprintRequest {
    #[schemars(description = "Board ID")]
    pub board_id: String,
    #[schemars(description = "Sprint 名称")]
    pub name: String,
    #[schemars(description = "开始时间")]
    pub start_date: String,
    #[schemars(description = "结束时间")]
    pub end_date: String,
    #[schemars(description = "Sprint 目标")]
    pub goal: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraUpdateSprintRequest {
    #[schemars(description = "Sprint ID")]
    pub sprint_id: String,
    #[schemars(description = "Sprint 名称")]
    pub name: Option<String>,
    #[schemars(description = "Sprint 状态")]
    pub state: Option<String>,
    #[schemars(description = "开始时间")]
    pub start_date: Option<String>,
    #[schemars(description = "结束时间")]
    pub end_date: Option<String>,
    #[schemars(description = "Sprint 目标")]
    pub goal: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraAddIssuesToSprintRequest {
    #[schemars(description = "Sprint ID")]
    pub sprint_id: String,
    #[schemars(description = "Issue key 列表")]
    pub issue_keys: Vec<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetServiceDeskForProjectRequest {
    #[schemars(description = "项目 key")]
    pub project_key: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetServiceDeskQueuesRequest {
    #[schemars(description = "Service desk ID")]
    pub service_desk_id: String,
    #[schemars(description = "分页起始位置")]
    pub start_at: Option<usize>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetQueueIssuesRequest {
    #[schemars(description = "Service desk ID")]
    pub service_desk_id: String,
    #[schemars(description = "Queue ID")]
    pub queue_id: String,
    #[schemars(description = "分页起始位置")]
    pub start_at: Option<usize>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraCreateVersionRequest {
    #[schemars(description = "项目 key")]
    pub project_key: String,
    #[schemars(description = "版本名称")]
    pub name: String,
    #[schemars(description = "开始日期")]
    pub start_date: Option<String>,
    #[schemars(description = "发布日期")]
    pub release_date: Option<String>,
    #[schemars(description = "版本描述")]
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraBatchCreateVersionsRequest {
    #[schemars(description = "项目 key")]
    pub project_key: String,
    #[schemars(description = "版本对象列表")]
    pub versions: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetIssueProformaFormsRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetProformaFormDetailsRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
    #[schemars(description = "表单 ID")]
    pub form_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraUpdateProformaFormAnswersRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
    #[schemars(description = "表单 ID")]
    pub form_id: String,
    #[schemars(description = "答案对象列表")]
    pub answers: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetIssueDatesRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
    #[schemars(description = "是否包含状态变更历史")]
    pub include_status_changes: Option<bool>,
    #[schemars(description = "是否包含状态停留时间汇总")]
    pub include_status_summary: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetIssueSlaRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
    #[schemars(description = "要计算的 SLA 指标列表")]
    pub metrics: Option<Vec<String>>,
    #[schemars(description = "是否只按工作时间计算")]
    pub working_hours_only: Option<bool>,
    #[schemars(description = "是否包含原始日期数据")]
    pub include_raw_dates: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetIssueDevelopmentInfoRequest {
    #[schemars(description = "Jira issue key")]
    pub issue_key: String,
    #[schemars(description = "应用类型过滤")]
    pub application_type: Option<String>,
    #[schemars(description = "数据类型过滤")]
    pub data_type: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JiraGetIssuesDevelopmentInfoRequest {
    #[schemars(description = "Jira issue key 列表")]
    pub issue_keys: Vec<String>,
    #[schemars(description = "应用类型过滤")]
    pub application_type: Option<String>,
    #[schemars(description = "数据类型过滤")]
    pub data_type: Option<String>,
}

// ========== Bitbucket Server/Data Center 相关数据结构 ==========

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketApiReadRequest {
    #[schemars(description = "Bitbucket Server REST 路径；默认补 /rest/api/1.0 前缀")]
    pub path: String,
    #[schemars(description = "查询参数对象")]
    pub query_params: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketApiWriteRequest {
    #[schemars(description = "Bitbucket Server REST 路径；默认补 /rest/api/1.0 前缀")]
    pub path: String,
    #[schemars(description = "请求体 JSON 对象")]
    pub body: serde_json::Value,
    #[schemars(description = "查询参数对象")]
    pub query_params: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketListProjectsRequest {
    #[schemars(description = "分页起始位置")]
    pub start: Option<usize>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
    #[schemars(description = "按项目名称/key 模糊过滤")]
    pub name: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketListRepositoriesRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT，仍无默认值时列出可访问仓库")]
    pub project_key: Option<String>,
    #[schemars(description = "分页起始位置")]
    pub start: Option<usize>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
    #[schemars(description = "按仓库名称/slug 模糊过滤")]
    pub name: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketRepositoryRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketListBranchesRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "分支过滤文本")]
    pub filter_text: Option<String>,
    #[schemars(description = "排序字段，例如 MODIFICATION")]
    pub order_by: Option<String>,
    #[schemars(description = "是否返回分支详情")]
    pub details: Option<bool>,
    #[schemars(description = "分页起始位置")]
    pub start: Option<usize>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketGetBranchRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "分支名，可传 main 或 refs/heads/main")]
    pub branch: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketListCommitsRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "结束 ref/commit")]
    pub until: Option<String>,
    #[schemars(description = "起始 ref/commit")]
    pub since: Option<String>,
    #[schemars(description = "文件路径过滤")]
    pub path: Option<String>,
    #[schemars(description = "分页起始位置")]
    pub start: Option<usize>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketGetCommitRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "commit id")]
    pub commit_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketGetFileRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "仓库内文件路径")]
    pub file_path: String,
    #[schemars(description = "branch/tag/commit；不传使用默认分支")]
    pub at: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketListPullRequestsRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR 状态，如 OPEN、MERGED、DECLINED、ALL")]
    pub state: Option<String>,
    #[schemars(description = "方向，如 INCOMING 或 OUTGOING")]
    pub direction: Option<String>,
    #[schemars(description = "按 ref 过滤")]
    pub at: Option<String>,
    #[schemars(description = "排序字段")]
    pub order: Option<String>,
    #[schemars(description = "分页起始位置")]
    pub start: Option<usize>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketCreatePullRequestRequest {
    #[schemars(description = "目标项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "目标仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR 标题")]
    pub title: String,
    #[schemars(description = "PR 描述")]
    pub description: Option<String>,
    #[schemars(description = "源分支")]
    pub source_branch: String,
    #[schemars(description = "目标分支")]
    pub target_branch: String,
    #[schemars(description = "源项目 key；跨仓库 PR 可传")]
    pub source_project_key: Option<String>,
    #[schemars(description = "源仓库 slug；跨仓库 PR 可传")]
    pub source_repo_slug: Option<String>,
    #[schemars(description = "reviewer 用户名列表")]
    pub reviewers: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketPullRequestRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR ID")]
    pub pull_request_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketUpdatePullRequestRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR ID")]
    pub pull_request_id: String,
    #[schemars(description = "新标题")]
    pub title: Option<String>,
    #[schemars(description = "新描述")]
    pub description: Option<String>,
    #[schemars(description = "新目标分支")]
    pub target_branch: Option<String>,
    #[schemars(description = "reviewer 用户名列表")]
    pub reviewers: Option<Vec<String>>,
    #[schemars(description = "PR version；不传会自动读取当前 version")]
    pub version: Option<i64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketVersionedPullRequestRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR ID")]
    pub pull_request_id: String,
    #[schemars(description = "PR version；不传会自动读取当前 version")]
    pub version: Option<i64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketMergePullRequestRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR ID")]
    pub pull_request_id: String,
    #[schemars(description = "PR version；不传会自动读取当前 version")]
    pub version: Option<i64>,
    #[schemars(description = "merge commit 消息")]
    pub message: Option<String>,
    #[schemars(description = "Bitbucket Server merge strategyId")]
    pub strategy_id: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketSetPullRequestStatusRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR ID")]
    pub pull_request_id: String,
    #[schemars(description = "用户 slug/name；不传则使用 BITBUCKET_USERNAME")]
    pub user_slug: Option<String>,
    #[schemars(description = "PR version；不传会自动读取当前 version")]
    pub version: Option<i64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketPullRequestPageRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR ID")]
    pub pull_request_id: String,
    #[schemars(description = "分页起始位置")]
    pub start: Option<usize>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketPullRequestCommentRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR ID")]
    pub pull_request_id: String,
    #[schemars(description = "评论 ID")]
    pub comment_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketAddPullRequestCommentRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR ID")]
    pub pull_request_id: String,
    #[schemars(description = "评论内容")]
    pub text: String,
    #[schemars(description = "Server anchor 对象，用于行内评论")]
    pub anchor: Option<serde_json::Value>,
    #[schemars(description = "父评论 ID，用于回复")]
    pub parent_id: Option<i64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketUpdatePullRequestCommentRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR ID")]
    pub pull_request_id: String,
    #[schemars(description = "评论 ID")]
    pub comment_id: String,
    #[schemars(description = "新评论内容")]
    pub text: String,
    #[schemars(description = "评论 version")]
    pub version: Option<i64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketDeletePullRequestCommentRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR ID")]
    pub pull_request_id: String,
    #[schemars(description = "评论 ID")]
    pub comment_id: String,
    #[schemars(description = "评论 version")]
    pub version: Option<i64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketSetPullRequestCommentStateRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR ID")]
    pub pull_request_id: String,
    #[schemars(description = "评论 ID")]
    pub comment_id: String,
    #[schemars(description = "评论 version；不传会先读取当前 comment")]
    pub version: Option<i64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketGetPullRequestDiffRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR ID")]
    pub pull_request_id: String,
    #[schemars(description = "可选文件路径")]
    pub path: Option<String>,
    #[schemars(description = "上下文行数")]
    pub context_lines: Option<usize>,
    #[schemars(description = "空白处理模式")]
    pub whitespace: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketGetPullRequestChangesRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR ID")]
    pub pull_request_id: String,
    #[schemars(description = "基准变更 ID")]
    pub since_id: Option<String>,
    #[schemars(description = "是否包含 comments")]
    pub with_comments: Option<bool>,
    #[schemars(description = "分页起始位置")]
    pub start: Option<usize>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketPullRequestTaskRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR ID")]
    pub pull_request_id: String,
    #[schemars(description = "任务 ID")]
    pub task_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketCreatePullRequestTaskRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR ID")]
    pub pull_request_id: String,
    #[schemars(description = "任务内容")]
    pub text: String,
    #[schemars(description = "Server anchor 对象，用于行内任务")]
    pub anchor: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketUpdatePullRequestTaskRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR ID")]
    pub pull_request_id: String,
    #[schemars(description = "任务 ID")]
    pub task_id: String,
    #[schemars(description = "任务内容")]
    pub text: Option<String>,
    #[schemars(description = "任务状态")]
    pub state: Option<String>,
    #[schemars(description = "任务 version")]
    pub version: Option<i64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketDeletePullRequestTaskRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR ID")]
    pub pull_request_id: String,
    #[schemars(description = "任务 ID")]
    pub task_id: String,
    #[schemars(description = "任务 version")]
    pub version: Option<i64>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketGetPullRequestStatusesRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "PR ID")]
    pub pull_request_id: String,
    #[schemars(description = "commit id；不传则使用 PR source latestCommit")]
    pub commit_id: Option<String>,
    #[schemars(description = "分页起始位置")]
    pub start: Option<usize>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
    #[schemars(description = "排序字段")]
    pub order_by: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketGetPendingReviewPullRequestsRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "当前 reviewer 用户 slug/name；不传则使用 BITBUCKET_USERNAME")]
    pub user_slug: Option<String>,
    #[schemars(description = "限定扫描的仓库 slug 列表；不传则扫描项目内仓库")]
    pub repository_list: Option<Vec<String>>,
    #[schemars(description = "最大返回 PR 数量")]
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketRepositoryBranchingModelRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketUpdateRepositoryBranchingModelRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "Bitbucket Server branchmodel configuration JSON")]
    pub body: serde_json::Value,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketProjectBranchingModelRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketUpdateProjectBranchingModelRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "Bitbucket Server branchmodel configuration JSON")]
    pub body: serde_json::Value,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketGetDefaultReviewersRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug；不传查询项目级默认 reviewer")]
    pub repo_slug: Option<String>,
    #[schemars(description = "源 ref id，例如 refs/heads/feature/x")]
    pub source_ref_id: Option<String>,
    #[schemars(description = "目标 ref id，例如 refs/heads/main")]
    pub target_ref_id: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct BitbucketCloneRepositoryRequest {
    #[schemars(description = "项目 key；不传则使用 BITBUCKET_PROJECT")]
    pub project_key: Option<String>,
    #[schemars(description = "仓库 slug")]
    pub repo_slug: String,
    #[schemars(description = "本地目标目录；会 clone 到 target_path/repo_slug")]
    pub target_path: String,
    #[schemars(description = "clone 协议，ssh 或 http/https")]
    pub protocol: Option<String>,
}

// ========== Confluence 相关数据结构 ==========

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceSearchRequest {
    #[schemars(description = "搜索查询，可传简单词条或 CQL")]
    pub query: String,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
    #[schemars(description = "空间过滤器，逗号分隔，例如 DEV,DOC")]
    pub spaces_filter: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceGetPageRequest {
    #[schemars(description = "页面 ID")]
    pub page_id: Option<String>,
    #[schemars(description = "页面标题")]
    pub title: Option<String>,
    #[schemars(description = "空间 key")]
    pub space_key: Option<String>,
    #[schemars(description = "是否包含元数据")]
    pub include_metadata: Option<bool>,
    #[schemars(description = "是否将内容转换为更易读的文本")]
    pub convert_to_markdown: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceGetPageChildrenRequest {
    #[schemars(description = "父页面 ID")]
    pub parent_id: String,
    #[schemars(description = "expand 参数")]
    pub expand: Option<String>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
    #[schemars(description = "是否包含子页面内容")]
    pub include_content: Option<bool>,
    #[schemars(description = "是否将内容转换为更易读的文本")]
    pub convert_to_markdown: Option<bool>,
    #[schemars(description = "起始偏移")]
    pub start: Option<usize>,
    #[schemars(description = "是否包含文件夹占位参数")]
    pub include_folders: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceGetCommentsRequest {
    #[schemars(description = "页面 ID")]
    pub page_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceGetLabelsRequest {
    #[schemars(description = "页面或内容 ID")]
    pub page_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceAddLabelRequest {
    #[schemars(description = "页面或内容 ID")]
    pub page_id: String,
    #[schemars(description = "标签名称")]
    pub name: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceCreatePageRequest {
    #[schemars(description = "空间 key")]
    pub space_key: String,
    #[schemars(description = "页面标题")]
    pub title: String,
    #[schemars(description = "页面内容")]
    pub content: String,
    #[schemars(description = "父页面 ID")]
    pub parent_id: Option<String>,
    #[schemars(description = "内容格式，可选 markdown/wiki/storage")]
    pub content_format: Option<String>,
    #[schemars(description = "是否启用标题锚点")]
    pub enable_heading_anchors: Option<bool>,
    #[schemars(description = "是否在响应中包含页面内容")]
    pub include_content: Option<bool>,
    #[schemars(description = "页面 emoji，占位参数")]
    pub emoji: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceUpdatePageRequest {
    #[schemars(description = "页面 ID")]
    pub page_id: String,
    #[schemars(description = "页面标题")]
    pub title: String,
    #[schemars(description = "页面内容")]
    pub content: String,
    #[schemars(description = "是否 minor edit")]
    pub is_minor_edit: Option<bool>,
    #[schemars(description = "版本备注")]
    pub version_comment: Option<String>,
    #[schemars(description = "新父页面 ID")]
    pub parent_id: Option<String>,
    #[schemars(description = "内容格式，可选 markdown/wiki/storage")]
    pub content_format: Option<String>,
    #[schemars(description = "是否启用标题锚点")]
    pub enable_heading_anchors: Option<bool>,
    #[schemars(description = "是否在响应中包含页面内容")]
    pub include_content: Option<bool>,
    #[schemars(description = "页面 emoji，占位参数")]
    pub emoji: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceDeletePageRequest {
    #[schemars(description = "页面 ID")]
    pub page_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceAddCommentRequest {
    #[schemars(description = "页面 ID")]
    pub page_id: String,
    #[schemars(description = "评论内容")]
    pub body: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceReplyToCommentRequest {
    #[schemars(description = "评论 ID")]
    pub comment_id: String,
    #[schemars(description = "回复内容")]
    pub body: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceSearchUserRequest {
    #[schemars(description = "用户搜索关键字")]
    pub query: String,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
    #[schemars(description = "Server/DC group 名称")]
    pub group_name: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceGetPageHistoryRequest {
    #[schemars(description = "页面 ID")]
    pub page_id: String,
    #[schemars(description = "版本号")]
    pub version: u32,
    #[schemars(description = "是否将内容转换为更易读的文本")]
    pub convert_to_markdown: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceGetPageViewsRequest {
    #[schemars(description = "页面 ID")]
    pub page_id: String,
    #[schemars(description = "是否包含标题")]
    pub include_title: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceGetSpacePageTreeRequest {
    #[schemars(description = "空间 key")]
    pub space_key: String,
    #[schemars(description = "最大抓取页面数")]
    pub limit: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceMovePageRequest {
    #[schemars(description = "页面 ID")]
    pub page_id: String,
    #[schemars(description = "目标父页面 ID")]
    pub target_parent_id: Option<String>,
    #[schemars(description = "目标空间 key")]
    pub target_space_key: Option<String>,
    #[schemars(description = "位置：append/above/below")]
    pub position: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceGetPageDiffRequest {
    #[schemars(description = "页面 ID")]
    pub page_id: String,
    #[schemars(description = "起始版本号")]
    pub from_version: u32,
    #[schemars(description = "目标版本号")]
    pub to_version: u32,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceUploadAttachmentRequest {
    #[schemars(description = "内容 ID")]
    pub content_id: String,
    #[schemars(description = "文件路径")]
    pub file_path: String,
    #[schemars(description = "附件备注")]
    pub comment: Option<String>,
    #[schemars(description = "是否 minor edit")]
    pub minor_edit: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceUploadAttachmentsRequest {
    #[schemars(description = "内容 ID")]
    pub content_id: String,
    #[schemars(description = "文件路径列表")]
    pub file_paths: Vec<String>,
    #[schemars(description = "附件备注")]
    pub comment: Option<String>,
    #[schemars(description = "是否 minor edit")]
    pub minor_edit: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceGetAttachmentsRequest {
    #[schemars(description = "内容 ID")]
    pub content_id: String,
    #[schemars(description = "起始偏移")]
    pub start: Option<usize>,
    #[schemars(description = "最大返回数量")]
    pub limit: Option<usize>,
    #[schemars(description = "按文件名精确过滤")]
    pub filename: Option<String>,
    #[schemars(description = "按 MIME 类型过滤")]
    pub media_type: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceDownloadAttachmentRequest {
    #[schemars(description = "附件 ID")]
    pub attachment_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceDownloadContentAttachmentsRequest {
    #[schemars(description = "内容 ID")]
    pub content_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceDeleteAttachmentRequest {
    #[schemars(description = "附件 ID")]
    pub attachment_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConfluenceGetPageImagesRequest {
    #[schemars(description = "内容 ID")]
    pub content_id: String,
}

// ========== 企业微信机器人请求结构体 ==========

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct WeixinSendTextRequest {
    #[schemars(description = "消息内容")]
    pub content: String,
    #[schemars(description = "提及的用户列表，如 [\"wangqing\", \"@all\"]")]
    pub mentioned_list: Option<Vec<String>>,
    #[schemars(description = "提及的手机号列表，如 [\"13800001111\", \"@all\"]")]
    pub mentioned_mobile_list: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct WeixinSendMarkdownRequest {
    #[schemars(description = "Markdown 内容")]
    pub content: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct WeixinSendImageRequest {
    #[schemars(description = "图片媒体 ID")]
    pub media_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct WeixinSendFileRequest {
    #[schemars(description = "文件媒体 ID")]
    pub media_id: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct WeixinArticle {
    #[schemars(description = "标题")]
    pub title: String,
    #[schemars(description = "描述")]
    pub description: String,
    #[schemars(description = "点击后跳转的链接")]
    pub url: String,
    #[schemars(description = "图片链接，可选")]
    pub picurl: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct WeixinSendNewsRequest {
    #[schemars(description = "图文消息文章列表")]
    pub articles: Vec<WeixinArticle>,
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

    fn serialize_value<T: Serialize>(value: T) -> String {
        serde_json::to_string(&value).unwrap_or_else(|_| "Failed to serialize".to_string())
    }

    fn serialize_result<T: Serialize>(result: Result<T, crate::searcher::SearcherError>) -> String {
        match result {
            Ok(value) => Self::serialize_value(value),
            Err(error) => format!("Error: {}", error),
        }
    }

    fn jira_not_configured() -> String {
        "Error: Jira client not configured. Please set JIRA_URL and either JIRA_PERSONAL_TOKEN or JIRA_USERNAME/JIRA_API_TOKEN.".to_string()
    }

    fn confluence_not_configured() -> String {
        "Error: Confluence client not configured. Please set CONFLUENCE_URL and either CONFLUENCE_PERSONAL_TOKEN or CONFLUENCE_USERNAME/CONFLUENCE_API_TOKEN.".to_string()
    }

    fn bitbucket_not_configured() -> String {
        "Error: Bitbucket Server client not configured. Please set BITBUCKET_URL and either BITBUCKET_PERSONAL_TOKEN or BITBUCKET_USERNAME/BITBUCKET_PASSWORD.".to_string()
    }

    fn object_param(
        value: Option<serde_json::Value>,
        field_name: &str,
    ) -> Result<serde_json::Map<String, serde_json::Value>, crate::searcher::SearcherError> {
        crate::searcher::atlassian::ensure_object(value, field_name)
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
    pub async fn harbor_get_project(
        &self,
        Parameters(params): Parameters<HarborGetProjectRequest>,
    ) -> String {
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
    pub async fn harbor_create_project(
        &self,
        Parameters(params): Parameters<HarborCreateProjectRequest>,
    ) -> String {
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
            }
            None => "Error: Harbor client not configured".to_string(),
        }
    }

    #[tool(description = "删除 Harbor 项目")]
    pub async fn harbor_delete_project(
        &self,
        Parameters(params): Parameters<HarborDeleteProjectRequest>,
    ) -> String {
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
    pub async fn harbor_get_repositories(
        &self,
        Parameters(params): Parameters<HarborGetRepositoriesRequest>,
    ) -> String {
        info!(
            "获取 Harbor 仓库列表: project={}",
            params.project_id_or_name
        );
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
    pub async fn harbor_delete_repository(
        &self,
        Parameters(params): Parameters<HarborDeleteRepositoryRequest>,
    ) -> String {
        info!(
            "删除 Harbor 仓库: project={}, repo={}",
            params.project_id_or_name, params.repository_name
        );
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
    pub async fn harbor_get_artifacts(
        &self,
        Parameters(params): Parameters<HarborGetArtifactsRequest>,
    ) -> String {
        info!(
            "获取 Harbor artifacts: project={}, repo={}",
            params.project_id_or_name, params.repository_name
        );
        match self.searcher.harbor() {
            Some(harbor) => match harbor
                .get_artifacts(&params.project_id_or_name, &params.repository_name)
                .await
            {
                Ok(artifacts) => serde_json::to_string(&artifacts)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Harbor client not configured".to_string(),
        }
    }

    #[tool(description = "删除 artifact (镜像标签)")]
    pub async fn harbor_delete_artifact(
        &self,
        Parameters(params): Parameters<HarborDeleteArtifactRequest>,
    ) -> String {
        info!(
            "删除 Harbor artifact: project={}, repo={}, digest={}",
            params.project_id_or_name, params.repository_name, params.digest
        );
        match self.searcher.harbor() {
            Some(harbor) => match harbor
                .delete_artifact(
                    &params.project_id_or_name,
                    &params.repository_name,
                    &params.digest,
                )
                .await
            {
                Ok(()) => serde_json::json!({
                    "status": "success",
                    "message": format!("Artifact '{}/{}@{}' deleted successfully",
                        params.project_id_or_name, params.repository_name, params.digest)
                })
                .to_string(),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Harbor client not configured".to_string(),
        }
    }

    #[tool(description = "获取项目的 Helm Charts 列表")]
    pub async fn harbor_get_helm_charts(
        &self,
        Parameters(params): Parameters<HarborGetHelmChartsRequest>,
    ) -> String {
        info!(
            "获取 Harbor Helm Charts: project={}",
            params.project_id_or_name
        );
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
    pub async fn harbor_get_helm_chart_versions(
        &self,
        Parameters(params): Parameters<HarborGetHelmChartVersionsRequest>,
    ) -> String {
        info!(
            "获取 Harbor Helm Chart 版本: project={}, chart={}",
            params.project_id_or_name, params.chart_name
        );
        match self.searcher.harbor() {
            Some(harbor) => match harbor
                .get_helm_chart_versions(&params.project_id_or_name, &params.chart_name)
                .await
            {
                Ok(versions) => serde_json::to_string(&versions)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Harbor client not configured".to_string(),
        }
    }

    #[tool(description = "删除 Helm Chart 版本")]
    pub async fn harbor_delete_helm_chart_version(
        &self,
        Parameters(params): Parameters<HarborDeleteHelmChartVersionRequest>,
    ) -> String {
        info!(
            "删除 Harbor Helm Chart 版本: project={}, chart={}, version={}",
            params.project_id_or_name, params.chart_name, params.version
        );
        match self.searcher.harbor() {
            Some(harbor) => match harbor
                .delete_helm_chart_version(
                    &params.project_id_or_name,
                    &params.chart_name,
                    &params.version,
                )
                .await
            {
                Ok(()) => serde_json::json!({
                    "status": "success",
                    "message": format!("Helm Chart '{}/{}:{}' deleted successfully",
                        params.project_id_or_name, params.chart_name, params.version)
                })
                .to_string(),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Harbor client not configured".to_string(),
        }
    }

    // ========== Nacos Tools ==========

    #[tool(description = "获取 Nacos 所有命名空间列表")]
    pub async fn nacos_list_namespaces(
        &self,
        _params: Parameters<NacosListNamespacesRequest>,
    ) -> String {
        info!("获取 Nacos 命名空间列表");
        match self.searcher.nacos() {
            Some(nacos) => match nacos.list_namespaces().await {
                Ok(namespaces) => serde_json::to_string(&namespaces)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => {
                "Error: Nacos client not configured. Please set NACOS_URL environment variable."
                    .to_string()
            }
        }
    }

    #[tool(description = "获取 Nacos 服务列表")]
    pub async fn nacos_list_services(
        &self,
        Parameters(params): Parameters<NacosListServicesRequest>,
    ) -> String {
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
            }
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Nacos 指定服务的详情")]
    pub async fn nacos_get_service(
        &self,
        Parameters(params): Parameters<NacosGetServiceRequest>,
    ) -> String {
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
            }
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Nacos 服务实例列表")]
    pub async fn nacos_list_instances(
        &self,
        Parameters(params): Parameters<NacosListInstancesRequest>,
    ) -> String {
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
            }
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Nacos 服务订阅者列表")]
    pub async fn nacos_list_service_subscribers(
        &self,
        Parameters(params): Parameters<NacosListServiceSubscribersRequest>,
    ) -> String {
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
            }
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Nacos 配置列表")]
    pub async fn nacos_list_configs(
        &self,
        Parameters(params): Parameters<NacosListConfigsRequest>,
    ) -> String {
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
            }
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Nacos 配置详情")]
    pub async fn nacos_get_config(
        &self,
        Parameters(params): Parameters<NacosGetConfigRequest>,
    ) -> String {
        info!(
            "获取 Nacos 配置详情: dataId={}, group={}",
            params.data_id, params.group_name
        );
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
            }
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Nacos 配置历史列表")]
    pub async fn nacos_list_config_history(
        &self,
        Parameters(params): Parameters<NacosListConfigHistoryRequest>,
    ) -> String {
        info!(
            "获取 Nacos 配置历史: dataId={}, group={}",
            params.data_id, params.group_name
        );
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
            }
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Nacos 配置历史详情")]
    pub async fn nacos_get_config_history(
        &self,
        Parameters(params): Parameters<NacosGetConfigHistoryRequest>,
    ) -> String {
        info!(
            "获取 Nacos 配置历史详情: dataId={}, group={}, nid={:?}",
            params.data_id, params.group_name, params.nid
        );
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
            }
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Nacos 配置监听器列表")]
    pub async fn nacos_list_config_listeners(
        &self,
        Parameters(params): Parameters<NacosListConfigListenersRequest>,
    ) -> String {
        info!(
            "获取 Nacos 配置监听器: dataId={}, group={}",
            params.data_id, params.group_name
        );
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
            }
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    #[tool(description = "获取客户端监听的 Nacos 配置列表")]
    pub async fn nacos_list_listened_configs(
        &self,
        Parameters(params): Parameters<NacosListListenedConfigsRequest>,
    ) -> String {
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
            }
            None => "Error: Nacos client not configured".to_string(),
        }
    }

    // ========== Kafka Tools ==========

    #[tool(description = "创建 Kafka 主题")]
    pub async fn kafka_create_topic(
        &self,
        Parameters(params): Parameters<KafkaCreateTopicRequest>,
    ) -> String {
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
    pub async fn kafka_delete_topic(
        &self,
        Parameters(params): Parameters<KafkaDeleteTopicRequest>,
    ) -> String {
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
    pub async fn kafka_describe_topic(
        &self,
        Parameters(params): Parameters<KafkaDescribeTopicRequest>,
    ) -> String {
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
    pub async fn kafka_produce_message(
        &self,
        Parameters(params): Parameters<KafkaProduceMessageRequest>,
    ) -> String {
        info!("生产消息到 Kafka 主题: {}", params.topic);
        match self.searcher.kafka() {
            Some(kafka) => match kafka
                .produce_message(
                    &params.topic,
                    params.key.clone(),
                    &params.value,
                    params.headers.clone(),
                )
                .await
            {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Kafka client not configured".to_string(),
        }
    }

    #[tool(description = "从 Kafka 主题消费消息")]
    pub async fn kafka_consume_messages(
        &self,
        Parameters(params): Parameters<KafkaConsumeMessagesRequest>,
    ) -> String {
        info!("从 Kafka 主题消费���息: {}", params.topic);
        match self.searcher.kafka() {
            Some(kafka) => match kafka
                .consume_messages(&params.topic, params.timeout_seconds)
                .await
            {
                Ok(messages) => serde_json::to_string(&messages)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Kafka client not configured".to_string(),
        }
    }

    // ========== Doris Tools ==========

    #[tool(description = "获取 Doris 所有数据库列表")]
    pub async fn doris_get_databases(
        &self,
        Parameters(params): Parameters<DorisGetDatabasesRequest>,
    ) -> String {
        info!("获取 Doris 数据库列表: catalog={:?}", params.catalog_name);
        match self.searcher.doris() {
            Some(doris) => match doris.get_databases_with_options(params.catalog_name.as_deref()).await {
                Ok(dbs) => serde_json::to_string(&dbs)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured. Please set DORIS_HOST, DORIS_USERNAME, DORIS_PASSWORD and DORIS_DB.".to_string(),
        }
    }

    #[tool(description = "获取 Doris Catalog 列表")]
    pub async fn doris_get_catalog_list(
        &self,
        _params: Parameters<DorisGetCatalogListRequest>,
    ) -> String {
        info!("获取 Doris Catalog 列表");
        match self.searcher.doris() {
            Some(doris) => match doris.get_catalog_list().await {
                Ok(catalogs) => serde_json::to_string(&catalogs)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Doris 最近审计日志")]
    pub async fn doris_get_recent_audit_logs(
        &self,
        Parameters(params): Parameters<DorisGetRecentAuditLogsRequest>,
    ) -> String {
        info!(
            "获取 Doris 最近审计日志: days={:?}, limit={:?}",
            params.days, params.limit
        );
        match self.searcher.doris() {
            Some(doris) => match doris.get_recent_audit_logs(params.days, params.limit).await {
                Ok(logs) => serde_json::to_string(&logs)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取指定数据库的表列表")]
    pub async fn doris_get_tables(
        &self,
        Parameters(params): Parameters<DorisGetTablesRequest>,
    ) -> String {
        info!(
            "获取 Doris 表列表: db={:?}, catalog={:?}",
            params.db_name, params.catalog_name
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .get_tables_with_options(params.db_name.as_deref(), params.catalog_name.as_deref())
                .await
            {
                Ok(tables) => serde_json::to_string(&tables)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取表结构详情")]
    pub async fn doris_get_table_schema(
        &self,
        Parameters(params): Parameters<DorisGetTableSchemaRequest>,
    ) -> String {
        info!(
            "获取 Doris 表结构: catalog={:?}, db={:?}, table={}",
            params.catalog_name, params.db_name, params.table_name
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .get_table_schema_with_options(
                    params.db_name.as_deref(),
                    &params.table_name,
                    params.catalog_name.as_deref(),
                )
                .await
            {
                Ok(schema) => serde_json::to_string(&schema)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取表元数据（大小、行数等）")]
    pub async fn doris_get_table_metadata(
        &self,
        Parameters(params): Parameters<DorisGetTableMetadataRequest>,
    ) -> String {
        info!(
            "获取 Doris 表元数据: catalog={:?}, db={:?}, table={}",
            params.catalog_name, params.db_name, params.table_name
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .get_table_metadata_with_options(
                    params.db_name.as_deref(),
                    &params.table_name,
                    params.catalog_name.as_deref(),
                )
                .await
            {
                Ok(metadata) => serde_json::to_string(&metadata)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Doris FE (Frontend) 节点状态")]
    pub async fn doris_get_fe_status(
        &self,
        _params: Parameters<DorisGetFeStatusRequest>,
    ) -> String {
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
    pub async fn doris_get_be_status(
        &self,
        _params: Parameters<DorisGetBeStatusRequest>,
    ) -> String {
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
    pub async fn doris_get_query_stats(
        &self,
        _params: Parameters<DorisGetQueryStatsRequest>,
    ) -> String {
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
    pub async fn doris_get_routine_loads(
        &self,
        _params: Parameters<DorisGetRoutineLoadsRequest>,
    ) -> String {
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
    pub async fn doris_get_load_jobs(
        &self,
        _params: Parameters<DorisGetLoadJobsRequest>,
    ) -> String {
        info!("获取 Doris Load 任���列表");
        match self.searcher.doris() {
            Some(doris) => match doris.get_load_jobs().await {
                Ok(jobs) => serde_json::to_string(&jobs)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "执行 SQL 查询")]
    pub async fn doris_exec_query(
        &self,
        Parameters(params): Parameters<DorisExecQueryRequest>,
    ) -> String {
        info!(
            "执行 Doris SQL 查询: sql={}, db={:?}, catalog={:?}, max_rows={:?}, timeout={:?}",
            params.sql, params.db_name, params.catalog_name, params.max_rows, params.timeout
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .execute_query_with_options(
                    &params.sql,
                    params.db_name.as_deref(),
                    params.catalog_name.as_deref(),
                    params.max_rows,
                    params.timeout,
                )
                .await
            {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取表注释信息")]
    pub async fn doris_get_table_comment(
        &self,
        Parameters(params): Parameters<DorisGetTableCommentRequest>,
    ) -> String {
        info!(
            "获取 Doris 表注释: catalog={:?}, db={:?}, table={}",
            params.catalog_name, params.db_name, params.table_name
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .get_table_comment(
                    params.db_name.as_deref(),
                    &params.table_name,
                    params.catalog_name.as_deref(),
                )
                .await
            {
                Ok(comment) => serde_json::to_string(&comment)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取表列注释信息")]
    pub async fn doris_get_table_column_comments(
        &self,
        Parameters(params): Parameters<DorisGetTableCommentRequest>,
    ) -> String {
        info!(
            "获取 Doris 表列注释: catalog={:?}, db={:?}, table={}",
            params.catalog_name, params.db_name, params.table_name
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .get_table_schema_with_options(
                    params.db_name.as_deref(),
                    &params.table_name,
                    params.catalog_name.as_deref(),
                )
                .await
            {
                Ok(schema) => {
                    let columns: Vec<serde_json::Value> = schema
                        .columns
                        .into_iter()
                        .map(|col| {
                            serde_json::json!({
                                "column_name": col.name,
                                "data_type": col.data_type,
                                "comment": col.comment.unwrap_or_else(|| "".to_string())
                            })
                        })
                        .collect();

                    serde_json::json!({
                        "catalog_name": params.catalog_name,
                        "database": params.db_name,
                        "table": params.table_name,
                        "columns": columns
                    })
                    .to_string()
                }
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取表索引信息")]
    pub async fn doris_get_table_indexes(
        &self,
        Parameters(params): Parameters<DorisGetTableIndexesRequest>,
    ) -> String {
        info!(
            "获取 Doris 表索引: catalog={:?}, db={:?}, table={}",
            params.catalog_name, params.db_name, params.table_name
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .get_table_indexes(
                    params.db_name.as_deref(),
                    &params.table_name,
                    params.catalog_name.as_deref(),
                )
                .await
            {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取 SQL 执行计划")]
    pub async fn doris_get_sql_explain(
        &self,
        Parameters(params): Parameters<DorisGetSqlExplainRequest>,
    ) -> String {
        info!(
            "获取 Doris SQL 执行计划: sql={}, db={:?}, catalog={:?}, verbose={:?}",
            params.sql, params.db_name, params.catalog_name, params.verbose
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .get_sql_explain(
                    &params.sql,
                    params.verbose.unwrap_or(false),
                    params.db_name.as_deref(),
                    params.catalog_name.as_deref(),
                )
                .await
            {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Doris SQL 执行 Profile")]
    pub async fn doris_get_sql_profile(
        &self,
        Parameters(params): Parameters<DorisGetSqlProfileRequest>,
    ) -> String {
        info!(
            "获取 Doris SQL Profile: sql={}, db={:?}, catalog={:?}, timeout={:?}",
            params.sql, params.db_name, params.catalog_name, params.timeout
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .get_sql_profile(
                    &params.sql,
                    params.db_name.as_deref(),
                    params.catalog_name.as_deref(),
                    params.timeout,
                )
                .await
            {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Doris 表数据大小信息")]
    pub async fn doris_get_table_data_size(
        &self,
        Parameters(params): Parameters<DorisGetTableDataSizeRequest>,
    ) -> String {
        info!(
            "获取 Doris 表数据大小: db={:?}, table={:?}, single_replica={:?}",
            params.db_name, params.table_name, params.single_replica
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .get_table_data_size(
                    params.db_name.as_deref(),
                    params.table_name.as_deref(),
                    params.single_replica.unwrap_or(false),
                )
                .await
            {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured. Please set DORIS_HTTP_URL environment variable.".to_string(),
        }
    }

    #[tool(description = "获取 Doris 内存统计信息")]
    pub async fn doris_get_memory_stats(
        &self,
        Parameters(params): Parameters<DorisGetMemoryStatsRequest>,
    ) -> String {
        info!(
            "获取 Doris 内存统计: data_type={:?}, tracker_type={:?}, tracker_names={:?}, time_range={:?}, include_details={:?}",
            params.data_type,
            params.tracker_type,
            params.tracker_names,
            params.time_range,
            params.include_details
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .get_memory_stats(
                    params.data_type.as_deref(),
                    params.tracker_type.as_deref(),
                    params.tracker_names.clone(),
                    params.time_range.as_deref(),
                    params.include_details.unwrap_or(true),
                )
                .await
            {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Doris 监控指标定义和/或数据")]
    pub async fn doris_get_monitoring_metrics(
        &self,
        Parameters(params): Parameters<DorisGetMonitoringMetricsRequest>,
    ) -> String {
        info!(
            "获取 Doris 监控指标: content_type={:?}, role={:?}, monitor_type={:?}, priority={:?}, include_raw_metrics={:?}",
            params.content_type,
            params.role,
            params.monitor_type,
            params.priority,
            params.include_raw_metrics
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .get_monitoring_metrics(
                    params.content_type.as_deref(),
                    params.role.as_deref(),
                    params.monitor_type.as_deref(),
                    params.priority.as_deref(),
                    params.include_raw_metrics.unwrap_or(false),
                )
                .await
            {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Doris 表基础信息")]
    pub async fn doris_get_table_basic_info(
        &self,
        Parameters(params): Parameters<DorisGetTableBasicInfoRequest>,
    ) -> String {
        info!(
            "获取 Doris 表基础信息: catalog={:?}, db={:?}, table={}",
            params.catalog_name, params.db_name, params.table_name
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .get_table_basic_info(
                    params.db_name.as_deref(),
                    &params.table_name,
                    params.catalog_name.as_deref(),
                )
                .await
            {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "分析 Doris 指定列的完整性和分布特征")]
    pub async fn doris_analyze_columns(
        &self,
        Parameters(params): Parameters<DorisAnalyzeColumnsRequest>,
    ) -> String {
        info!(
            "分析 Doris 列: catalog={:?}, db={:?}, table={}, columns={:?}, analysis_types={:?}, sample_size={:?}, detailed_response={:?}",
            params.catalog_name,
            params.db_name,
            params.table_name,
            params.columns,
            params.analysis_types,
            params.sample_size,
            params.detailed_response
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .analyze_columns(
                    params.db_name.as_deref(),
                    &params.table_name,
                    &params.columns,
                    params.analysis_types.as_deref(),
                    params.sample_size,
                    params.catalog_name.as_deref(),
                    params.detailed_response.unwrap_or(false),
                )
                .await
            {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "分析 Doris 表的物理存储、分桶和分区信息")]
    pub async fn doris_analyze_table_storage(
        &self,
        Parameters(params): Parameters<DorisAnalyzeTableStorageRequest>,
    ) -> String {
        info!(
            "分析 Doris 表存储: catalog={:?}, db={:?}, table={}, detailed_response={:?}",
            params.catalog_name, params.db_name, params.table_name, params.detailed_response
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .analyze_table_storage(
                    params.db_name.as_deref(),
                    &params.table_name,
                    params.catalog_name.as_deref(),
                    params.detailed_response.unwrap_or(false),
                )
                .await
            {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "追踪 Doris 列级血缘关系")]
    pub async fn doris_trace_column_lineage(
        &self,
        Parameters(params): Parameters<DorisTraceColumnLineageRequest>,
    ) -> String {
        info!(
            "追踪 Doris 列血缘: target_columns={:?}, analysis_depth={:?}, include_transformations={:?}, catalog={:?}",
            params.target_columns,
            params.analysis_depth,
            params.include_transformations,
            params.catalog_name
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .trace_column_lineage(
                    &params.target_columns,
                    params.analysis_depth,
                    params.include_transformations.unwrap_or(true),
                    params.catalog_name.as_deref(),
                )
                .await
            {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "监控 Doris 表数据新鲜度")]
    pub async fn doris_monitor_data_freshness(
        &self,
        Parameters(params): Parameters<DorisMonitorDataFreshnessRequest>,
    ) -> String {
        info!(
            "监控 Doris 数据新鲜度: catalog={:?}, db={:?}, table_names={:?}, freshness_threshold_hours={:?}, include_update_patterns={:?}",
            params.catalog_name,
            params.db_name,
            params.table_names,
            params.freshness_threshold_hours,
            params.include_update_patterns
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .monitor_data_freshness(
                    params.db_name.as_deref(),
                    params.table_names.as_deref(),
                    params.freshness_threshold_hours.unwrap_or(24),
                    params.include_update_patterns.unwrap_or(true),
                    params.catalog_name.as_deref(),
                )
                .await
            {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "分析 Doris 用户数据访问模式和安全风险")]
    pub async fn doris_analyze_data_access_patterns(
        &self,
        Parameters(params): Parameters<DorisAnalyzeDataAccessPatternsRequest>,
    ) -> String {
        info!(
            "分析 Doris 数据访问模式: days={:?}, include_system_users={:?}, min_query_threshold={:?}",
            params.days, params.include_system_users, params.min_query_threshold
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .analyze_data_access_patterns(
                    params.days,
                    params.include_system_users.unwrap_or(false),
                    params.min_query_threshold,
                )
                .await
            {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "分析 Doris 表级数据流依赖关系")]
    pub async fn doris_analyze_data_flow_dependencies(
        &self,
        Parameters(params): Parameters<DorisAnalyzeDataFlowDependenciesRequest>,
    ) -> String {
        info!(
            "分析 Doris 数据流依赖: catalog={:?}, db={:?}, target_table={:?}, analysis_depth={:?}, include_views={:?}",
            params.catalog_name,
            params.db_name,
            params.target_table,
            params.analysis_depth,
            params.include_views
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .analyze_data_flow_dependencies(
                    params.db_name.as_deref(),
                    params.target_table.as_deref(),
                    params.analysis_depth,
                    params.include_views.unwrap_or(true),
                    params.catalog_name.as_deref(),
                )
                .await
            {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "分析 Doris 最慢查询 Top N")]
    pub async fn doris_analyze_slow_queries_topn(
        &self,
        Parameters(params): Parameters<DorisAnalyzeSlowQueriesTopNRequest>,
    ) -> String {
        info!(
            "分析 Doris 慢查询: days={:?}, top_n={:?}, min_execution_time_ms={:?}, include_patterns={:?}",
            params.days, params.top_n, params.min_execution_time_ms, params.include_patterns
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .analyze_slow_queries_topn(
                    params.days,
                    params.top_n,
                    params.min_execution_time_ms,
                    params.include_patterns.unwrap_or(true),
                )
                .await
            {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "分析 Doris 资源增长趋势")]
    pub async fn doris_analyze_resource_growth_curves(
        &self,
        Parameters(params): Parameters<DorisAnalyzeResourceGrowthCurvesRequest>,
    ) -> String {
        info!(
            "分析 Doris 资源增长趋势: days={:?}, resource_types={:?}, include_predictions={:?}, detailed_response={:?}",
            params.days,
            params.resource_types,
            params.include_predictions,
            params.detailed_response
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .analyze_resource_growth_curves(
                    params.days,
                    params.resource_types.as_deref(),
                    params.include_predictions.unwrap_or(false),
                    params.detailed_response.unwrap_or(false),
                )
                .await
            {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "执行 Doris ADBC 兼容查询（当前 Rust 版本会回退到 MySQL 查询通道）")]
    pub async fn doris_exec_adbc_query(
        &self,
        Parameters(params): Parameters<DorisExecAdbcQueryRequest>,
    ) -> String {
        info!(
            "执行 Doris ADBC 兼容查询: sql={}, max_rows={:?}, timeout={:?}, return_format={:?}",
            params.sql, params.max_rows, params.timeout, params.return_format
        );
        match self.searcher.doris() {
            Some(doris) => match doris
                .exec_adbc_query(
                    &params.sql,
                    params.max_rows,
                    params.timeout,
                    params.return_format.as_deref(),
                )
                .await
            {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    #[tool(description = "获取 Doris ADBC / Arrow Flight SQL 连接诊断信息")]
    pub async fn doris_get_adbc_connection_info(
        &self,
        _params: Parameters<DorisGetAdbcConnectionInfoRequest>,
    ) -> String {
        info!("获取 Doris ADBC 连接诊断信息");
        match self.searcher.doris() {
            Some(doris) => match doris.get_adbc_connection_info().await {
                Ok(result) => serde_json::to_string(&result)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Doris client not configured".to_string(),
        }
    }

    // ========== Jira Tools ==========

    #[tool(description = "获取 Jira issue 详情")]
    pub async fn jira_get_issue(
        &self,
        Parameters(params): Parameters<JiraGetIssueRequest>,
    ) -> String {
        info!("获取 Jira issue: {}", params.issue_key);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.get_issue(
                    &params.issue_key,
                    params.fields.as_deref(),
                    params.expand.as_deref(),
                    params.comment_limit,
                    params.properties.as_deref(),
                    params.update_history,
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "使用 JQL 搜索 Jira issues")]
    pub async fn jira_search(&self, Parameters(params): Parameters<JiraSearchRequest>) -> String {
        info!("搜索 Jira issues: {}", params.jql);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.search_issues(
                    &params.jql,
                    params.fields.as_deref(),
                    params.start_at,
                    params.limit,
                    params.projects_filter.as_deref(),
                    params.expand.as_deref(),
                    params.page_token.as_deref(),
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "搜索 Jira 字段定义")]
    pub async fn jira_search_fields(
        &self,
        Parameters(params): Parameters<JiraSearchFieldsRequest>,
    ) -> String {
        info!("搜索 Jira 字段: {:?}", params.keyword);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.search_fields(params.keyword.as_deref(), params.limit)
                    .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira 字段允许的选项值")]
    pub async fn jira_get_field_options(
        &self,
        Parameters(params): Parameters<JiraGetFieldOptionsRequest>,
    ) -> String {
        info!("获取 Jira 字段选项: {}", params.field_id);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.get_field_options(
                    &params.field_id,
                    params.context_id.as_deref(),
                    params.project_key.as_deref(),
                    params.issue_type.as_deref(),
                    params.contains.as_deref(),
                    params.return_limit,
                    params.values_only,
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取指定 Jira 项目的 issues")]
    pub async fn jira_get_project_issues(
        &self,
        Parameters(params): Parameters<JiraGetProjectIssuesRequest>,
    ) -> String {
        info!("获取 Jira 项目 issues: {}", params.project_key);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.get_project_issues(
                    &params.project_key,
                    params.fields.as_deref(),
                    params.start_at,
                    params.limit,
                    params.expand.as_deref(),
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira issue 可用状态流转")]
    pub async fn jira_get_transitions(
        &self,
        Parameters(params): Parameters<JiraGetTransitionsRequest>,
    ) -> String {
        info!("获取 Jira transitions: {}", params.issue_key);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(jira.get_transitions(&params.issue_key).await),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira issue worklog")]
    pub async fn jira_get_worklog(
        &self,
        Parameters(params): Parameters<JiraGetWorklogRequest>,
    ) -> String {
        info!("获取 Jira worklog: {}", params.issue_key);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(jira.get_worklog(&params.issue_key).await),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira 项目版本列表")]
    pub async fn jira_get_project_versions(
        &self,
        Parameters(params): Parameters<JiraGetProjectVersionsRequest>,
    ) -> String {
        info!("获取 Jira 项目版本: {}", params.project_key);
        match self.searcher.jira() {
            Some(jira) => {
                Self::serialize_result(jira.get_project_versions(&params.project_key).await)
            }
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira 项目组件列表")]
    pub async fn jira_get_project_components(
        &self,
        Parameters(params): Parameters<JiraGetProjectComponentsRequest>,
    ) -> String {
        info!("获取 Jira 项目组件: {}", params.project_key);
        match self.searcher.jira() {
            Some(jira) => {
                Self::serialize_result(jira.get_project_components(&params.project_key).await)
            }
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取当前用户可访问的 Jira 项目列表")]
    pub async fn jira_get_all_projects(
        &self,
        Parameters(params): Parameters<JiraGetAllProjectsRequest>,
    ) -> String {
        info!("获取全部 Jira 项目");
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.get_all_projects(params.include_archived.unwrap_or(false))
                    .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira 用户资料")]
    pub async fn jira_get_user_profile(
        &self,
        Parameters(params): Parameters<JiraGetUserProfileRequest>,
    ) -> String {
        info!("获取 Jira 用户资料: {}", params.user_identifier);
        match self.searcher.jira() {
            Some(jira) => {
                Self::serialize_result(jira.get_user_profile(&params.user_identifier).await)
            }
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira issue watchers")]
    pub async fn jira_get_issue_watchers(
        &self,
        Parameters(params): Parameters<JiraGetIssueWatchersRequest>,
    ) -> String {
        info!("获取 Jira watchers: {}", params.issue_key);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(jira.get_issue_watchers(&params.issue_key).await),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "为 Jira issue 添加 watcher")]
    pub async fn jira_add_watcher(
        &self,
        Parameters(params): Parameters<JiraAddWatcherRequest>,
    ) -> String {
        info!(
            "添加 Jira watcher: {} -> {}",
            params.issue_key, params.user_identifier
        );
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.add_watcher(&params.issue_key, &params.user_identifier)
                    .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "从 Jira issue 移除 watcher")]
    pub async fn jira_remove_watcher(
        &self,
        Parameters(params): Parameters<JiraRemoveWatcherRequest>,
    ) -> String {
        info!("移除 Jira watcher: {}", params.issue_key);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.remove_watcher(
                    &params.issue_key,
                    params.username.as_deref(),
                    params.account_id.as_deref(),
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "创建 Jira issue")]
    pub async fn jira_create_issue(
        &self,
        Parameters(params): Parameters<JiraCreateIssueRequest>,
    ) -> String {
        info!(
            "创建 Jira issue: {} / {}",
            params.project_key, params.summary
        );
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.create_issue(
                    &params.project_key,
                    &params.summary,
                    &params.issue_type,
                    params.assignee.as_deref(),
                    params.description.as_deref(),
                    params.components.as_deref(),
                    params.additional_fields,
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "更新 Jira issue")]
    pub async fn jira_update_issue(
        &self,
        Parameters(params): Parameters<JiraUpdateIssueRequest>,
    ) -> String {
        info!("更新 Jira issue: {}", params.issue_key);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.update_issue(
                    &params.issue_key,
                    params.fields,
                    params.additional_fields,
                    params.components.as_deref(),
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "删除 Jira issue")]
    pub async fn jira_delete_issue(
        &self,
        Parameters(params): Parameters<JiraDeleteIssueRequest>,
    ) -> String {
        info!("删除 Jira issue: {}", params.issue_key);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(jira.delete_issue(&params.issue_key).await),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "向 Jira issue 添加评论")]
    pub async fn jira_add_comment(
        &self,
        Parameters(params): Parameters<JiraAddCommentRequest>,
    ) -> String {
        info!("添加 Jira 评论: {}", params.issue_key);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.add_comment(&params.issue_key, &params.comment, params.visibility)
                    .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "向 Jira issue 添加 worklog")]
    pub async fn jira_add_worklog(
        &self,
        Parameters(params): Parameters<JiraAddWorklogRequest>,
    ) -> String {
        info!("添加 Jira worklog: {}", params.issue_key);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.add_worklog(
                    &params.issue_key,
                    &params.time_spent,
                    params.comment.as_deref(),
                    params.started.as_deref(),
                    params.original_estimate.as_deref(),
                    params.remaining_estimate.as_deref(),
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "流转 Jira issue 状态")]
    pub async fn jira_transition_issue(
        &self,
        Parameters(params): Parameters<JiraTransitionIssueRequest>,
    ) -> String {
        info!(
            "流转 Jira issue: {} -> {}",
            params.issue_key, params.transition_id
        );
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.transition_issue(
                    &params.issue_key,
                    &params.transition_id,
                    params.fields,
                    params.comment.as_deref(),
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira issue link types")]
    pub async fn jira_get_link_types(
        &self,
        _params: Parameters<JiraGetLinkTypesRequest>,
    ) -> String {
        info!("获取 Jira link types");
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(jira.get_link_types().await),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "下载 Jira issue 的所有附件内容")]
    pub async fn jira_download_attachments(
        &self,
        Parameters(params): Parameters<JiraDownloadAttachmentsRequest>,
    ) -> String {
        info!("下载 Jira 附件: {}", params.issue_key);
        match self.searcher.jira() {
            Some(jira) => {
                Self::serialize_result(jira.download_attachments(&params.issue_key).await)
            }
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira issue 中的图片附件")]
    pub async fn jira_get_issue_images(
        &self,
        Parameters(params): Parameters<JiraGetIssueImagesRequest>,
    ) -> String {
        info!("获取 Jira 图片附件: {}", params.issue_key);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(jira.get_issue_images(&params.issue_key).await),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira Agile boards 列表")]
    pub async fn jira_get_agile_boards(
        &self,
        Parameters(params): Parameters<JiraGetAgileBoardsRequest>,
    ) -> String {
        info!("获取 Jira boards: {:?}", params.board_name);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.get_agile_boards(
                    params.board_name.as_deref(),
                    params.project_key.as_deref(),
                    params.board_type.as_deref(),
                    params.start_at,
                    params.limit,
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira board 下的 issues")]
    pub async fn jira_get_board_issues(
        &self,
        Parameters(params): Parameters<JiraGetBoardIssuesRequest>,
    ) -> String {
        info!("获取 Jira board issues: {}", params.board_id);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.get_board_issues(
                    &params.board_id,
                    &params.jql,
                    params.fields.as_deref(),
                    params.start_at,
                    params.limit,
                    params.expand.as_deref(),
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira board 下的 sprints")]
    pub async fn jira_get_sprints_from_board(
        &self,
        Parameters(params): Parameters<JiraGetSprintsFromBoardRequest>,
    ) -> String {
        info!("获取 Jira sprints: {}", params.board_id);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.get_sprints_from_board(
                    &params.board_id,
                    params.state.as_deref(),
                    params.start_at,
                    params.limit,
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira sprint 下的 issues")]
    pub async fn jira_get_sprint_issues(
        &self,
        Parameters(params): Parameters<JiraGetSprintIssuesRequest>,
    ) -> String {
        info!("获取 Jira sprint issues: {}", params.sprint_id);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.get_sprint_issues(
                    &params.sprint_id,
                    params.fields.as_deref(),
                    params.start_at,
                    params.limit,
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "批量创建 Jira issues")]
    pub async fn jira_batch_create_issues(
        &self,
        Parameters(params): Parameters<JiraBatchCreateIssuesRequest>,
    ) -> String {
        info!("批量创建 Jira issues: {}", params.issues.len());
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.batch_create_issues(&params.issues, params.validate_only.unwrap_or(false))
                    .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "批量获取 Jira issues 的 changelogs")]
    pub async fn jira_batch_get_changelogs(
        &self,
        Parameters(params): Parameters<JiraBatchGetChangelogsRequest>,
    ) -> String {
        info!(
            "批量获取 Jira changelogs: {}",
            params.issue_ids_or_keys.len()
        );
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.batch_get_changelogs(
                    &params.issue_ids_or_keys,
                    params.fields.as_deref(),
                    params.limit,
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "编辑 Jira 评论")]
    pub async fn jira_edit_comment(
        &self,
        Parameters(params): Parameters<JiraEditCommentRequest>,
    ) -> String {
        info!(
            "编辑 Jira 评论: {} / {}",
            params.issue_key, params.comment_id
        );
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.edit_comment(
                    &params.issue_key,
                    &params.comment_id,
                    &params.body,
                    params.visibility,
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "将 Jira issue 关联到 Epic")]
    pub async fn jira_link_to_epic(
        &self,
        Parameters(params): Parameters<JiraLinkToEpicRequest>,
    ) -> String {
        info!(
            "关联 Jira Epic: {} -> {}",
            params.issue_key, params.epic_key
        );
        match self.searcher.jira() {
            Some(jira) => {
                Self::serialize_result(jira.link_to_epic(&params.issue_key, &params.epic_key).await)
            }
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "创建 Jira issue link")]
    pub async fn jira_create_issue_link(
        &self,
        Parameters(params): Parameters<JiraCreateIssueLinkRequest>,
    ) -> String {
        info!(
            "创建 Jira issue link: {} <-> {}",
            params.inward_issue_key, params.outward_issue_key
        );
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.create_issue_link(
                    &params.link_type,
                    &params.inward_issue_key,
                    &params.outward_issue_key,
                    params.comment.as_deref(),
                    params.comment_visibility,
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "创建 Jira 远程 issue link")]
    pub async fn jira_create_remote_issue_link(
        &self,
        Parameters(params): Parameters<JiraCreateRemoteIssueLinkRequest>,
    ) -> String {
        info!("创建 Jira 远程链接: {}", params.issue_key);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.create_remote_issue_link(
                    &params.issue_key,
                    &params.url,
                    &params.title,
                    params.summary.as_deref(),
                    params.relationship.as_deref(),
                    params.icon_url.as_deref(),
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "删除 Jira issue link")]
    pub async fn jira_remove_issue_link(
        &self,
        Parameters(params): Parameters<JiraRemoveIssueLinkRequest>,
    ) -> String {
        info!("删除 Jira issue link: {}", params.link_id);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(jira.remove_issue_link(&params.link_id).await),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "创建 Jira sprint")]
    pub async fn jira_create_sprint(
        &self,
        Parameters(params): Parameters<JiraCreateSprintRequest>,
    ) -> String {
        info!("创建 Jira sprint: {}", params.name);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.create_sprint(
                    &params.board_id,
                    &params.name,
                    &params.start_date,
                    &params.end_date,
                    params.goal.as_deref(),
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "更新 Jira sprint")]
    pub async fn jira_update_sprint(
        &self,
        Parameters(params): Parameters<JiraUpdateSprintRequest>,
    ) -> String {
        info!("更新 Jira sprint: {}", params.sprint_id);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.update_sprint(
                    &params.sprint_id,
                    params.name.as_deref(),
                    params.state.as_deref(),
                    params.start_date.as_deref(),
                    params.end_date.as_deref(),
                    params.goal.as_deref(),
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "将 issues 加入 Jira sprint")]
    pub async fn jira_add_issues_to_sprint(
        &self,
        Parameters(params): Parameters<JiraAddIssuesToSprintRequest>,
    ) -> String {
        info!("Sprint 加入 issues: {}", params.sprint_id);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.add_issues_to_sprint(&params.sprint_id, &params.issue_keys)
                    .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "按项目获取 Jira Service Desk")]
    pub async fn jira_get_service_desk_for_project(
        &self,
        Parameters(params): Parameters<JiraGetServiceDeskForProjectRequest>,
    ) -> String {
        info!("获取 Jira Service Desk: {}", params.project_key);
        match self.searcher.jira() {
            Some(jira) => {
                Self::serialize_result(jira.get_service_desk_for_project(&params.project_key).await)
            }
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira Service Desk queues")]
    pub async fn jira_get_service_desk_queues(
        &self,
        Parameters(params): Parameters<JiraGetServiceDeskQueuesRequest>,
    ) -> String {
        info!("获取 Jira queues: {}", params.service_desk_id);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.get_service_desk_queues(
                    &params.service_desk_id,
                    params.start_at,
                    params.limit,
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira queue 下的 issues")]
    pub async fn jira_get_queue_issues(
        &self,
        Parameters(params): Parameters<JiraGetQueueIssuesRequest>,
    ) -> String {
        info!(
            "获取 Jira queue issues: {} / {}",
            params.service_desk_id, params.queue_id
        );
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.get_queue_issues(
                    &params.service_desk_id,
                    &params.queue_id,
                    params.start_at,
                    params.limit,
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "创建 Jira 版本")]
    pub async fn jira_create_version(
        &self,
        Parameters(params): Parameters<JiraCreateVersionRequest>,
    ) -> String {
        info!("创建 Jira 版本: {} / {}", params.project_key, params.name);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.create_version(
                    &params.project_key,
                    &params.name,
                    params.start_date.as_deref(),
                    params.release_date.as_deref(),
                    params.description.as_deref(),
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "批量创建 Jira 版本")]
    pub async fn jira_batch_create_versions(
        &self,
        Parameters(params): Parameters<JiraBatchCreateVersionsRequest>,
    ) -> String {
        info!("批量创建 Jira 版本: {}", params.versions.len());
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.batch_create_versions(&params.project_key, &params.versions)
                    .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira issue 的 ProForma 表单列表")]
    pub async fn jira_get_issue_proforma_forms(
        &self,
        Parameters(params): Parameters<JiraGetIssueProformaFormsRequest>,
    ) -> String {
        info!("获取 Jira forms: {}", params.issue_key);
        match self.searcher.jira() {
            Some(jira) => {
                Self::serialize_result(jira.get_issue_proforma_forms(&params.issue_key).await)
            }
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira ProForma 表单详情")]
    pub async fn jira_get_proforma_form_details(
        &self,
        Parameters(params): Parameters<JiraGetProformaFormDetailsRequest>,
    ) -> String {
        info!(
            "获取 Jira form 详情: {} / {}",
            params.issue_key, params.form_id
        );
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.get_proforma_form_details(&params.issue_key, &params.form_id)
                    .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "更新 Jira ProForma 表单答案")]
    pub async fn jira_update_proforma_form_answers(
        &self,
        Parameters(params): Parameters<JiraUpdateProformaFormAnswersRequest>,
    ) -> String {
        info!(
            "更新 Jira form 答案: {} / {}",
            params.issue_key, params.form_id
        );
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.update_proforma_form_answers(
                    &params.issue_key,
                    &params.form_id,
                    &params.answers,
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira issue 的日期与状态历史")]
    pub async fn jira_get_issue_dates(
        &self,
        Parameters(params): Parameters<JiraGetIssueDatesRequest>,
    ) -> String {
        info!("获取 Jira issue 日期: {}", params.issue_key);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.get_issue_dates(
                    &params.issue_key,
                    params.include_status_changes.unwrap_or(true),
                    params.include_status_summary.unwrap_or(true),
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "计算 Jira issue SLA 指标")]
    pub async fn jira_get_issue_sla(
        &self,
        Parameters(params): Parameters<JiraGetIssueSlaRequest>,
    ) -> String {
        info!("计算 Jira SLA: {}", params.issue_key);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.get_issue_sla(
                    &params.issue_key,
                    params.metrics.as_deref(),
                    params.working_hours_only,
                    params.include_raw_dates.unwrap_or(false),
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "获取 Jira issue 的开发信息")]
    pub async fn jira_get_issue_development_info(
        &self,
        Parameters(params): Parameters<JiraGetIssueDevelopmentInfoRequest>,
    ) -> String {
        info!("获取 Jira 开发信息: {}", params.issue_key);
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.get_issue_development_info(
                    &params.issue_key,
                    params.application_type.as_deref(),
                    params.data_type.as_deref(),
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    #[tool(description = "批量获取 Jira issues 的开发信息")]
    pub async fn jira_get_issues_development_info(
        &self,
        Parameters(params): Parameters<JiraGetIssuesDevelopmentInfoRequest>,
    ) -> String {
        info!("批量获取 Jira 开发信息: {}", params.issue_keys.len());
        match self.searcher.jira() {
            Some(jira) => Self::serialize_result(
                jira.get_issues_development_info(
                    &params.issue_keys,
                    params.application_type.as_deref(),
                    params.data_type.as_deref(),
                )
                .await,
            ),
            None => Self::jira_not_configured(),
        }
    }

    // ========== Bitbucket Server/Data Center Tools ==========

    #[tool(description = "Bitbucket Server 通用 GET 请求；默认使用 /rest/api/1.0")]
    pub async fn bitbucket_get(
        &self,
        Parameters(params): Parameters<BitbucketApiReadRequest>,
    ) -> String {
        info!("Bitbucket GET: {}", params.path);
        match self.searcher.bitbucket() {
            Some(bitbucket) => match Self::object_param(params.query_params, "query_params") {
                Ok(query_params) => {
                    Self::serialize_result(bitbucket.get(&params.path, Some(query_params)).await)
                }
                Err(error) => format!("Error: {}", error),
            },
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "Bitbucket Server 通用 POST 请求；默认使用 /rest/api/1.0")]
    pub async fn bitbucket_post(
        &self,
        Parameters(params): Parameters<BitbucketApiWriteRequest>,
    ) -> String {
        info!("Bitbucket POST: {}", params.path);
        match self.searcher.bitbucket() {
            Some(bitbucket) => match Self::object_param(params.query_params, "query_params") {
                Ok(query_params) => Self::serialize_result(
                    bitbucket
                        .post(&params.path, Some(query_params), params.body)
                        .await,
                ),
                Err(error) => format!("Error: {}", error),
            },
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "Bitbucket Server 通用 PUT 请求；默认使用 /rest/api/1.0")]
    pub async fn bitbucket_put(
        &self,
        Parameters(params): Parameters<BitbucketApiWriteRequest>,
    ) -> String {
        info!("Bitbucket PUT: {}", params.path);
        match self.searcher.bitbucket() {
            Some(bitbucket) => match Self::object_param(params.query_params, "query_params") {
                Ok(query_params) => Self::serialize_result(
                    bitbucket
                        .put(&params.path, Some(query_params), params.body)
                        .await,
                ),
                Err(error) => format!("Error: {}", error),
            },
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "Bitbucket Server 通用 PATCH 请求；默认使用 /rest/api/1.0")]
    pub async fn bitbucket_patch(
        &self,
        Parameters(params): Parameters<BitbucketApiWriteRequest>,
    ) -> String {
        info!("Bitbucket PATCH: {}", params.path);
        match self.searcher.bitbucket() {
            Some(bitbucket) => match Self::object_param(params.query_params, "query_params") {
                Ok(query_params) => Self::serialize_result(
                    bitbucket
                        .patch(&params.path, Some(query_params), params.body)
                        .await,
                ),
                Err(error) => format!("Error: {}", error),
            },
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "Bitbucket Server 通用 DELETE 请求；默认使用 /rest/api/1.0")]
    pub async fn bitbucket_delete(
        &self,
        Parameters(params): Parameters<BitbucketApiReadRequest>,
    ) -> String {
        info!("Bitbucket DELETE: {}", params.path);
        match self.searcher.bitbucket() {
            Some(bitbucket) => match Self::object_param(params.query_params, "query_params") {
                Ok(query_params) => {
                    Self::serialize_result(bitbucket.delete(&params.path, Some(query_params)).await)
                }
                Err(error) => format!("Error: {}", error),
            },
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "列出 Bitbucket Server 项目")]
    pub async fn bitbucket_list_projects(
        &self,
        Parameters(params): Parameters<BitbucketListProjectsRequest>,
    ) -> String {
        info!("Bitbucket list projects");
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .list_projects(params.start, params.limit, params.name.as_deref())
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "列出 Bitbucket Server 仓库")]
    pub async fn bitbucket_list_repositories(
        &self,
        Parameters(params): Parameters<BitbucketListRepositoriesRequest>,
    ) -> String {
        info!("Bitbucket list repositories: {:?}", params.project_key);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .list_repositories(
                        params.project_key.as_deref(),
                        params.start,
                        params.limit,
                        params.name.as_deref(),
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "获取 Bitbucket Server 仓库详情")]
    pub async fn bitbucket_get_repository(
        &self,
        Parameters(params): Parameters<BitbucketRepositoryRequest>,
    ) -> String {
        info!("Bitbucket get repository: {}", params.repo_slug);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_repository(params.project_key.as_deref(), &params.repo_slug)
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "列出 Bitbucket Server 仓库分支")]
    pub async fn bitbucket_list_branches(
        &self,
        Parameters(params): Parameters<BitbucketListBranchesRequest>,
    ) -> String {
        info!("Bitbucket list branches: {}", params.repo_slug);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .list_branches(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        params.filter_text.as_deref(),
                        params.order_by.as_deref(),
                        params.details,
                        params.start,
                        params.limit,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "获取 Bitbucket Server 分支详情")]
    pub async fn bitbucket_get_branch(
        &self,
        Parameters(params): Parameters<BitbucketGetBranchRequest>,
    ) -> String {
        info!(
            "Bitbucket get branch: {} / {}",
            params.repo_slug, params.branch
        );
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_branch(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.branch,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "列出 Bitbucket Server 仓库 commits")]
    pub async fn bitbucket_list_commits(
        &self,
        Parameters(params): Parameters<BitbucketListCommitsRequest>,
    ) -> String {
        info!("Bitbucket list commits: {}", params.repo_slug);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .list_commits(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        params.until.as_deref(),
                        params.since.as_deref(),
                        params.path.as_deref(),
                        params.start,
                        params.limit,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "获取 Bitbucket Server commit 详情")]
    pub async fn bitbucket_get_commit(
        &self,
        Parameters(params): Parameters<BitbucketGetCommitRequest>,
    ) -> String {
        info!("Bitbucket get commit: {}", params.commit_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_commit(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.commit_id,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "读取 Bitbucket Server 仓库文件内容")]
    pub async fn bitbucket_get_file_content(
        &self,
        Parameters(params): Parameters<BitbucketGetFileRequest>,
    ) -> String {
        info!(
            "Bitbucket get file: {} / {}",
            params.repo_slug, params.file_path
        );
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_file_content(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.file_path,
                        params.at.as_deref(),
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "列出 Bitbucket Server Pull Requests")]
    pub async fn bitbucket_list_pull_requests(
        &self,
        Parameters(params): Parameters<BitbucketListPullRequestsRequest>,
    ) -> String {
        info!("Bitbucket list PRs: {}", params.repo_slug);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .list_pull_requests(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        params.state.as_deref(),
                        params.direction.as_deref(),
                        params.at.as_deref(),
                        params.order.as_deref(),
                        params.start,
                        params.limit,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "创建 Bitbucket Server Pull Request")]
    pub async fn bitbucket_create_pull_request(
        &self,
        Parameters(params): Parameters<BitbucketCreatePullRequestRequest>,
    ) -> String {
        info!("Bitbucket create PR: {}", params.title);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .create_pull_request(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.title,
                        params.description.as_deref(),
                        &params.source_branch,
                        &params.target_branch,
                        params.source_project_key.as_deref(),
                        params.source_repo_slug.as_deref(),
                        params.reviewers,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "获取 Bitbucket Server Pull Request 详情")]
    pub async fn bitbucket_get_pull_request(
        &self,
        Parameters(params): Parameters<BitbucketPullRequestRequest>,
    ) -> String {
        info!("Bitbucket get PR: {}", params.pull_request_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_pull_request(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "更新 Bitbucket Server Pull Request")]
    pub async fn bitbucket_update_pull_request(
        &self,
        Parameters(params): Parameters<BitbucketUpdatePullRequestRequest>,
    ) -> String {
        info!("Bitbucket update PR: {}", params.pull_request_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .update_pull_request(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        params.title.as_deref(),
                        params.description.as_deref(),
                        params.target_branch.as_deref(),
                        params.reviewers,
                        params.version,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "合并 Bitbucket Server Pull Request")]
    pub async fn bitbucket_merge_pull_request(
        &self,
        Parameters(params): Parameters<BitbucketMergePullRequestRequest>,
    ) -> String {
        info!("Bitbucket merge PR: {}", params.pull_request_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .merge_pull_request(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        params.version,
                        params.message.as_deref(),
                        params.strategy_id.as_deref(),
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "Decline Bitbucket Server Pull Request")]
    pub async fn bitbucket_decline_pull_request(
        &self,
        Parameters(params): Parameters<BitbucketVersionedPullRequestRequest>,
    ) -> String {
        info!("Bitbucket decline PR: {}", params.pull_request_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .decline_pull_request(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        params.version,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "Reopen Bitbucket Server Pull Request")]
    pub async fn bitbucket_reopen_pull_request(
        &self,
        Parameters(params): Parameters<BitbucketVersionedPullRequestRequest>,
    ) -> String {
        info!("Bitbucket reopen PR: {}", params.pull_request_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .reopen_pull_request(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        params.version,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "Approve Bitbucket Server Pull Request")]
    pub async fn bitbucket_approve_pull_request(
        &self,
        Parameters(params): Parameters<BitbucketSetPullRequestStatusRequest>,
    ) -> String {
        info!("Bitbucket approve PR: {}", params.pull_request_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .set_pull_request_status(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        params.user_slug.as_deref(),
                        "APPROVED",
                        params.version,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "取消 Bitbucket Server Pull Request approve")]
    pub async fn bitbucket_unapprove_pull_request(
        &self,
        Parameters(params): Parameters<BitbucketSetPullRequestStatusRequest>,
    ) -> String {
        info!("Bitbucket unapprove PR: {}", params.pull_request_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .set_pull_request_status(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        params.user_slug.as_deref(),
                        "UNAPPROVED",
                        params.version,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "在 Bitbucket Server Pull Request 上请求修改")]
    pub async fn bitbucket_request_changes(
        &self,
        Parameters(params): Parameters<BitbucketSetPullRequestStatusRequest>,
    ) -> String {
        info!("Bitbucket request changes: {}", params.pull_request_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .set_pull_request_status(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        params.user_slug.as_deref(),
                        "NEEDS_WORK",
                        params.version,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "获取 Bitbucket Server Pull Request activity")]
    pub async fn bitbucket_get_pull_request_activity(
        &self,
        Parameters(params): Parameters<BitbucketPullRequestPageRequest>,
    ) -> String {
        info!("Bitbucket PR activity: {}", params.pull_request_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_pull_request_activity(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        params.start,
                        params.limit,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "列出 Bitbucket Server Pull Request commits")]
    pub async fn bitbucket_get_pull_request_commits(
        &self,
        Parameters(params): Parameters<BitbucketPullRequestPageRequest>,
    ) -> String {
        info!("Bitbucket PR commits: {}", params.pull_request_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_pull_request_commits(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        params.start,
                        params.limit,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "列出 Bitbucket Server Pull Request comments")]
    pub async fn bitbucket_get_pull_request_comments(
        &self,
        Parameters(params): Parameters<BitbucketPullRequestPageRequest>,
    ) -> String {
        info!("Bitbucket PR comments: {}", params.pull_request_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_pull_request_comments(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        params.start,
                        params.limit,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "获取 Bitbucket Server Pull Request 单条 comment")]
    pub async fn bitbucket_get_pull_request_comment(
        &self,
        Parameters(params): Parameters<BitbucketPullRequestCommentRequest>,
    ) -> String {
        info!("Bitbucket get PR comment: {}", params.comment_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_pull_request_comment(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        &params.comment_id,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "添加 Bitbucket Server Pull Request comment")]
    pub async fn bitbucket_add_pull_request_comment(
        &self,
        Parameters(params): Parameters<BitbucketAddPullRequestCommentRequest>,
    ) -> String {
        info!("Bitbucket add PR comment: {}", params.pull_request_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .add_pull_request_comment(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        &params.text,
                        params.anchor,
                        params.parent_id,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "更新 Bitbucket Server Pull Request comment")]
    pub async fn bitbucket_update_pull_request_comment(
        &self,
        Parameters(params): Parameters<BitbucketUpdatePullRequestCommentRequest>,
    ) -> String {
        info!("Bitbucket update PR comment: {}", params.comment_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .update_pull_request_comment(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        &params.comment_id,
                        &params.text,
                        params.version,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "删除 Bitbucket Server Pull Request comment")]
    pub async fn bitbucket_delete_pull_request_comment(
        &self,
        Parameters(params): Parameters<BitbucketDeletePullRequestCommentRequest>,
    ) -> String {
        info!("Bitbucket delete PR comment: {}", params.comment_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .delete_pull_request_comment(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        &params.comment_id,
                        params.version,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "Resolve Bitbucket Server Pull Request comment thread")]
    pub async fn bitbucket_resolve_comment(
        &self,
        Parameters(params): Parameters<BitbucketSetPullRequestCommentStateRequest>,
    ) -> String {
        info!("Bitbucket resolve PR comment: {}", params.comment_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .set_pull_request_comment_state(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        &params.comment_id,
                        "RESOLVED",
                        params.version,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "Reopen Bitbucket Server Pull Request comment thread")]
    pub async fn bitbucket_reopen_comment(
        &self,
        Parameters(params): Parameters<BitbucketSetPullRequestCommentStateRequest>,
    ) -> String {
        info!("Bitbucket reopen PR comment: {}", params.comment_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .set_pull_request_comment_state(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        &params.comment_id,
                        "OPEN",
                        params.version,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "获取 Bitbucket Server Pull Request diff")]
    pub async fn bitbucket_get_pull_request_diff(
        &self,
        Parameters(params): Parameters<BitbucketGetPullRequestDiffRequest>,
    ) -> String {
        info!("Bitbucket PR diff: {}", params.pull_request_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_pull_request_diff(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        params.path.as_deref(),
                        params.context_lines,
                        params.whitespace.as_deref(),
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(
        description = "获取 Bitbucket Server Pull Request changed files；对应 Cloud diffstat 的 Server/DC 版本"
    )]
    pub async fn bitbucket_get_pull_request_changes(
        &self,
        Parameters(params): Parameters<BitbucketGetPullRequestChangesRequest>,
    ) -> String {
        info!("Bitbucket PR changes: {}", params.pull_request_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_pull_request_changes(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        params.since_id.as_deref(),
                        params.with_comments,
                        params.start,
                        params.limit,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "获取 Bitbucket Server Pull Request patch")]
    pub async fn bitbucket_get_pull_request_patch(
        &self,
        Parameters(params): Parameters<BitbucketPullRequestRequest>,
    ) -> String {
        info!("Bitbucket PR patch: {}", params.pull_request_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_pull_request_patch(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "列出 Bitbucket Server Pull Request tasks")]
    pub async fn bitbucket_list_pull_request_tasks(
        &self,
        Parameters(params): Parameters<BitbucketPullRequestPageRequest>,
    ) -> String {
        info!("Bitbucket PR tasks: {}", params.pull_request_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .list_pull_request_tasks(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        params.start,
                        params.limit,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "获取 Bitbucket Server Pull Request 单个 task")]
    pub async fn bitbucket_get_pull_request_task(
        &self,
        Parameters(params): Parameters<BitbucketPullRequestTaskRequest>,
    ) -> String {
        info!("Bitbucket get PR task: {}", params.task_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_pull_request_task(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        &params.task_id,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "创建 Bitbucket Server Pull Request task")]
    pub async fn bitbucket_create_pull_request_task(
        &self,
        Parameters(params): Parameters<BitbucketCreatePullRequestTaskRequest>,
    ) -> String {
        info!("Bitbucket create PR task: {}", params.pull_request_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .create_pull_request_task(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        &params.text,
                        params.anchor,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "更新 Bitbucket Server Pull Request task")]
    pub async fn bitbucket_update_pull_request_task(
        &self,
        Parameters(params): Parameters<BitbucketUpdatePullRequestTaskRequest>,
    ) -> String {
        info!("Bitbucket update PR task: {}", params.task_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .update_pull_request_task(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        &params.task_id,
                        params.text.as_deref(),
                        params.state.as_deref(),
                        params.version,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "删除 Bitbucket Server Pull Request task")]
    pub async fn bitbucket_delete_pull_request_task(
        &self,
        Parameters(params): Parameters<BitbucketDeletePullRequestTaskRequest>,
    ) -> String {
        info!("Bitbucket delete PR task: {}", params.task_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .delete_pull_request_task(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        &params.task_id,
                        params.version,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "获取 Bitbucket Server Pull Request source commit 的 build statuses")]
    pub async fn bitbucket_get_pull_request_statuses(
        &self,
        Parameters(params): Parameters<BitbucketGetPullRequestStatusesRequest>,
    ) -> String {
        info!("Bitbucket PR statuses: {}", params.pull_request_id);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_pull_request_statuses(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.pull_request_id,
                        params.commit_id.as_deref(),
                        params.start,
                        params.limit,
                        params.order_by.as_deref(),
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(
        description = "列出当前用户在 Bitbucket Server 中待 review 且未 approve 的 Pull Requests"
    )]
    pub async fn bitbucket_get_pending_review_pull_requests(
        &self,
        Parameters(params): Parameters<BitbucketGetPendingReviewPullRequestsRequest>,
    ) -> String {
        info!("Bitbucket pending review PRs: {:?}", params.project_key);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_pending_review_pull_requests(
                        params.project_key.as_deref(),
                        params.user_slug.as_deref(),
                        params.repository_list,
                        params.limit,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "获取 Bitbucket Server 仓库 branching model")]
    pub async fn bitbucket_get_repository_branching_model(
        &self,
        Parameters(params): Parameters<BitbucketRepositoryBranchingModelRequest>,
    ) -> String {
        info!("Bitbucket repository branching model: {}", params.repo_slug);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_repository_branching_model(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "获取 Bitbucket Server 仓库 effective branching model")]
    pub async fn bitbucket_get_effective_repository_branching_model(
        &self,
        Parameters(params): Parameters<BitbucketRepositoryBranchingModelRequest>,
    ) -> String {
        info!(
            "Bitbucket effective repository branching model: {}",
            params.repo_slug
        );
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_effective_repository_branching_model(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "获取 Bitbucket Server 仓库 branching model configuration")]
    pub async fn bitbucket_get_repository_branching_model_settings(
        &self,
        Parameters(params): Parameters<BitbucketRepositoryBranchingModelRequest>,
    ) -> String {
        info!(
            "Bitbucket repository branching model settings: {}",
            params.repo_slug
        );
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_repository_branching_model_settings(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "更新 Bitbucket Server 仓库 branching model configuration")]
    pub async fn bitbucket_update_repository_branching_model_settings(
        &self,
        Parameters(params): Parameters<BitbucketUpdateRepositoryBranchingModelRequest>,
    ) -> String {
        info!(
            "Bitbucket update repository branching model settings: {}",
            params.repo_slug
        );
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .update_repository_branching_model_settings(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        params.body,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "获取 Bitbucket Server 项目 branching model")]
    pub async fn bitbucket_get_project_branching_model(
        &self,
        Parameters(params): Parameters<BitbucketProjectBranchingModelRequest>,
    ) -> String {
        info!(
            "Bitbucket project branching model: {:?}",
            params.project_key
        );
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_project_branching_model(params.project_key.as_deref())
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "获取 Bitbucket Server 项目 branching model configuration")]
    pub async fn bitbucket_get_project_branching_model_settings(
        &self,
        Parameters(params): Parameters<BitbucketProjectBranchingModelRequest>,
    ) -> String {
        info!(
            "Bitbucket project branching model settings: {:?}",
            params.project_key
        );
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_project_branching_model_settings(params.project_key.as_deref())
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "更新 Bitbucket Server 项目 branching model configuration")]
    pub async fn bitbucket_update_project_branching_model_settings(
        &self,
        Parameters(params): Parameters<BitbucketUpdateProjectBranchingModelRequest>,
    ) -> String {
        info!(
            "Bitbucket update project branching model settings: {:?}",
            params.project_key
        );
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .update_project_branching_model_settings(
                        params.project_key.as_deref(),
                        params.body,
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "获取 Bitbucket Server 默认 reviewer")]
    pub async fn bitbucket_get_default_reviewers(
        &self,
        Parameters(params): Parameters<BitbucketGetDefaultReviewersRequest>,
    ) -> String {
        info!("Bitbucket default reviewers: {:?}", params.project_key);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .get_default_reviewers(
                        params.project_key.as_deref(),
                        params.repo_slug.as_deref(),
                        params.source_ref_id.as_deref(),
                        params.target_ref_id.as_deref(),
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    #[tool(description = "clone Bitbucket Server 仓库到本地目录")]
    pub async fn bitbucket_clone_repository(
        &self,
        Parameters(params): Parameters<BitbucketCloneRepositoryRequest>,
    ) -> String {
        info!("Bitbucket clone repository: {}", params.repo_slug);
        match self.searcher.bitbucket() {
            Some(bitbucket) => Self::serialize_result(
                bitbucket
                    .clone_repository(
                        params.project_key.as_deref(),
                        &params.repo_slug,
                        &params.target_path,
                        params.protocol.as_deref(),
                    )
                    .await,
            ),
            None => Self::bitbucket_not_configured(),
        }
    }

    // ========== Confluence Tools ==========

    #[tool(description = "搜索 Confluence 内容")]
    pub async fn confluence_search(
        &self,
        Parameters(params): Parameters<ConfluenceSearchRequest>,
    ) -> String {
        info!("搜索 Confluence: {}", params.query);
        match self.searcher.confluence() {
            Some(confluence) => Self::serialize_result(
                confluence
                    .search(&params.query, params.limit, params.spaces_filter.as_deref())
                    .await,
            ),
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "获取 Confluence 页面内容")]
    pub async fn confluence_get_page(
        &self,
        Parameters(params): Parameters<ConfluenceGetPageRequest>,
    ) -> String {
        info!("获取 Confluence 页面: {:?}", params.page_id);
        match self.searcher.confluence() {
            Some(confluence) => Self::serialize_result(
                confluence
                    .get_page(
                        params.page_id.as_deref(),
                        params.title.as_deref(),
                        params.space_key.as_deref(),
                        params.include_metadata.unwrap_or(true),
                        params.convert_to_markdown.unwrap_or(true),
                    )
                    .await,
            ),
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "获取 Confluence 页面子页面")]
    pub async fn confluence_get_page_children(
        &self,
        Parameters(params): Parameters<ConfluenceGetPageChildrenRequest>,
    ) -> String {
        info!("获取 Confluence 子页面: {}", params.parent_id);
        match self.searcher.confluence() {
            Some(confluence) => Self::serialize_result(
                confluence
                    .get_page_children(
                        &params.parent_id,
                        params.expand.as_deref(),
                        params.limit,
                        params.include_content,
                        params.convert_to_markdown,
                        params.start,
                        params.include_folders,
                    )
                    .await,
            ),
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "获取 Confluence 页面评论")]
    pub async fn confluence_get_comments(
        &self,
        Parameters(params): Parameters<ConfluenceGetCommentsRequest>,
    ) -> String {
        info!("获取 Confluence 评论: {}", params.page_id);
        match self.searcher.confluence() {
            Some(confluence) => {
                Self::serialize_result(confluence.get_comments(&params.page_id).await)
            }
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "获取 Confluence 页面标签")]
    pub async fn confluence_get_labels(
        &self,
        Parameters(params): Parameters<ConfluenceGetLabelsRequest>,
    ) -> String {
        info!("获取 Confluence 标签: {}", params.page_id);
        match self.searcher.confluence() {
            Some(confluence) => {
                Self::serialize_result(confluence.get_labels(&params.page_id).await)
            }
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "为 Confluence 页面添加标签")]
    pub async fn confluence_add_label(
        &self,
        Parameters(params): Parameters<ConfluenceAddLabelRequest>,
    ) -> String {
        info!(
            "添加 Confluence 标签: {} -> {}",
            params.page_id, params.name
        );
        match self.searcher.confluence() {
            Some(confluence) => {
                Self::serialize_result(confluence.add_label(&params.page_id, &params.name).await)
            }
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "创建 Confluence 页面")]
    pub async fn confluence_create_page(
        &self,
        Parameters(params): Parameters<ConfluenceCreatePageRequest>,
    ) -> String {
        info!(
            "创建 Confluence 页面: {} / {}",
            params.space_key, params.title
        );
        match self.searcher.confluence() {
            Some(confluence) => Self::serialize_result(
                confluence
                    .create_page(
                        &params.space_key,
                        &params.title,
                        &params.content,
                        params.parent_id.as_deref(),
                        params.content_format.as_deref(),
                        params.enable_heading_anchors,
                        params.include_content,
                        params.emoji.as_deref(),
                    )
                    .await,
            ),
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "更新 Confluence 页面")]
    pub async fn confluence_update_page(
        &self,
        Parameters(params): Parameters<ConfluenceUpdatePageRequest>,
    ) -> String {
        info!("更新 Confluence 页面: {}", params.page_id);
        match self.searcher.confluence() {
            Some(confluence) => Self::serialize_result(
                confluence
                    .update_page(
                        &params.page_id,
                        &params.title,
                        &params.content,
                        params.is_minor_edit,
                        params.version_comment.as_deref(),
                        params.parent_id.as_deref(),
                        params.content_format.as_deref(),
                        params.enable_heading_anchors,
                        params.include_content,
                        params.emoji.as_deref(),
                    )
                    .await,
            ),
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "删除 Confluence 页面")]
    pub async fn confluence_delete_page(
        &self,
        Parameters(params): Parameters<ConfluenceDeletePageRequest>,
    ) -> String {
        info!("删除 Confluence 页面: {}", params.page_id);
        match self.searcher.confluence() {
            Some(confluence) => {
                Self::serialize_result(confluence.delete_page(&params.page_id).await)
            }
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "为 Confluence 页面添加评论")]
    pub async fn confluence_add_comment(
        &self,
        Parameters(params): Parameters<ConfluenceAddCommentRequest>,
    ) -> String {
        info!("添加 Confluence 评论: {}", params.page_id);
        match self.searcher.confluence() {
            Some(confluence) => {
                Self::serialize_result(confluence.add_comment(&params.page_id, &params.body).await)
            }
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "回复 Confluence 评论")]
    pub async fn confluence_reply_to_comment(
        &self,
        Parameters(params): Parameters<ConfluenceReplyToCommentRequest>,
    ) -> String {
        info!("回复 Confluence 评论: {}", params.comment_id);
        match self.searcher.confluence() {
            Some(confluence) => Self::serialize_result(
                confluence
                    .reply_to_comment(&params.comment_id, &params.body)
                    .await,
            ),
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "搜索 Confluence 用户")]
    pub async fn confluence_search_user(
        &self,
        Parameters(params): Parameters<ConfluenceSearchUserRequest>,
    ) -> String {
        info!("搜索 Confluence 用户: {}", params.query);
        match self.searcher.confluence() {
            Some(confluence) => Self::serialize_result(
                confluence
                    .search_user(&params.query, params.limit, params.group_name.as_deref())
                    .await,
            ),
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "获取 Confluence 历史版本页面")]
    pub async fn confluence_get_page_history(
        &self,
        Parameters(params): Parameters<ConfluenceGetPageHistoryRequest>,
    ) -> String {
        info!(
            "获取 Confluence 页面历史: {} @ {}",
            params.page_id, params.version
        );
        match self.searcher.confluence() {
            Some(confluence) => Self::serialize_result(
                confluence
                    .get_page_history(
                        &params.page_id,
                        params.version,
                        params.convert_to_markdown.unwrap_or(true),
                    )
                    .await,
            ),
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "获取 Confluence 页面浏览量")]
    pub async fn confluence_get_page_views(
        &self,
        Parameters(params): Parameters<ConfluenceGetPageViewsRequest>,
    ) -> String {
        info!("获取 Confluence 页面浏览量: {}", params.page_id);
        match self.searcher.confluence() {
            Some(confluence) => Self::serialize_result(
                confluence
                    .get_page_views(&params.page_id, params.include_title.unwrap_or(true))
                    .await,
            ),
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "获取 Confluence 空间页面树")]
    pub async fn confluence_get_space_page_tree(
        &self,
        Parameters(params): Parameters<ConfluenceGetSpacePageTreeRequest>,
    ) -> String {
        info!("获取 Confluence 页面树: {}", params.space_key);
        match self.searcher.confluence() {
            Some(confluence) => Self::serialize_result(
                confluence
                    .get_space_page_tree(&params.space_key, params.limit)
                    .await,
            ),
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "移动 Confluence 页面")]
    pub async fn confluence_move_page(
        &self,
        Parameters(params): Parameters<ConfluenceMovePageRequest>,
    ) -> String {
        info!("移动 Confluence 页面: {}", params.page_id);
        match self.searcher.confluence() {
            Some(confluence) => Self::serialize_result(
                confluence
                    .move_page(
                        &params.page_id,
                        params.target_parent_id.as_deref(),
                        params.target_space_key.as_deref(),
                        params.position.as_deref(),
                    )
                    .await,
            ),
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "比较 Confluence 页面两个版本的差异")]
    pub async fn confluence_get_page_diff(
        &self,
        Parameters(params): Parameters<ConfluenceGetPageDiffRequest>,
    ) -> String {
        info!(
            "获取 Confluence 页面 diff: {} ({} -> {})",
            params.page_id, params.from_version, params.to_version
        );
        match self.searcher.confluence() {
            Some(confluence) => Self::serialize_result(
                confluence
                    .get_page_diff(&params.page_id, params.from_version, params.to_version)
                    .await,
            ),
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "向 Confluence 页面上传附件")]
    pub async fn confluence_upload_attachment(
        &self,
        Parameters(params): Parameters<ConfluenceUploadAttachmentRequest>,
    ) -> String {
        info!(
            "上传 Confluence 附件: {} -> {}",
            params.content_id, params.file_path
        );
        match self.searcher.confluence() {
            Some(confluence) => Self::serialize_result(
                confluence
                    .upload_attachment(
                        &params.content_id,
                        &params.file_path,
                        params.comment.as_deref(),
                        params.minor_edit,
                    )
                    .await,
            ),
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "批量向 Confluence 页面上传附件")]
    pub async fn confluence_upload_attachments(
        &self,
        Parameters(params): Parameters<ConfluenceUploadAttachmentsRequest>,
    ) -> String {
        info!("批量上传 Confluence 附件: {}", params.content_id);
        match self.searcher.confluence() {
            Some(confluence) => Self::serialize_result(
                confluence
                    .upload_attachments(
                        &params.content_id,
                        &params.file_paths,
                        params.comment.as_deref(),
                        params.minor_edit,
                    )
                    .await,
            ),
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "获取 Confluence 内容附件列表")]
    pub async fn confluence_get_attachments(
        &self,
        Parameters(params): Parameters<ConfluenceGetAttachmentsRequest>,
    ) -> String {
        info!("获取 Confluence 附件: {}", params.content_id);
        match self.searcher.confluence() {
            Some(confluence) => Self::serialize_result(
                confluence
                    .get_attachments(
                        &params.content_id,
                        params.start,
                        params.limit,
                        params.filename.as_deref(),
                        params.media_type.as_deref(),
                    )
                    .await,
            ),
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "下载 Confluence 单个附件内容")]
    pub async fn confluence_download_attachment(
        &self,
        Parameters(params): Parameters<ConfluenceDownloadAttachmentRequest>,
    ) -> String {
        info!("下载 Confluence 附件: {}", params.attachment_id);
        match self.searcher.confluence() {
            Some(confluence) => {
                Self::serialize_result(confluence.download_attachment(&params.attachment_id).await)
            }
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "下载 Confluence 页面下的全部附件内容")]
    pub async fn confluence_download_content_attachments(
        &self,
        Parameters(params): Parameters<ConfluenceDownloadContentAttachmentsRequest>,
    ) -> String {
        info!("下载 Confluence 内容全部附件: {}", params.content_id);
        match self.searcher.confluence() {
            Some(confluence) => Self::serialize_result(
                confluence
                    .download_content_attachments(&params.content_id)
                    .await,
            ),
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "删除 Confluence 附件")]
    pub async fn confluence_delete_attachment(
        &self,
        Parameters(params): Parameters<ConfluenceDeleteAttachmentRequest>,
    ) -> String {
        info!("删除 Confluence 附件: {}", params.attachment_id);
        match self.searcher.confluence() {
            Some(confluence) => {
                Self::serialize_result(confluence.delete_attachment(&params.attachment_id).await)
            }
            None => Self::confluence_not_configured(),
        }
    }

    #[tool(description = "获取 Confluence 页面中的图片附件")]
    pub async fn confluence_get_page_images(
        &self,
        Parameters(params): Parameters<ConfluenceGetPageImagesRequest>,
    ) -> String {
        info!("获取 Confluence 页面图片: {}", params.content_id);
        match self.searcher.confluence() {
            Some(confluence) => {
                Self::serialize_result(confluence.get_page_images(&params.content_id).await)
            }
            None => Self::confluence_not_configured(),
        }
    }

    // ========== Kubernetes Tools ==========

    #[tool(description = "列出 Kubernetes Pod")]
    pub async fn kube_list_pods(
        &self,
        Parameters(params): Parameters<KubeListPodsRequest>,
    ) -> String {
        info!("列出 Kubernetes Pods: namespace={:?}", params.namespace);
        match self.searcher.kubernetes() {
            Some(kube) => match kube.list_pods(params.namespace.as_deref()).await {
                Ok(pods) => serde_json::to_string(&pods)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Kubernetes client not configured. Make sure you have a valid kubeconfig file.".to_string(),
        }
    }

    #[tool(description = "获取指定的 Kubernetes Pod")]
    pub async fn kube_get_pod(&self, Parameters(params): Parameters<KubeGetPodRequest>) -> String {
        info!("获取 Kubernetes Pod: {}", params.name);
        match self.searcher.kubernetes() {
            Some(kube) => match kube
                .get_pod(&params.name, params.namespace.as_deref())
                .await
            {
                Ok(pod) => serde_json::to_string(&pod)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Kubernetes client not configured".to_string(),
        }
    }

    #[tool(description = "删除 Kubernetes Pod")]
    pub async fn kube_delete_pod(
        &self,
        Parameters(params): Parameters<KubeDeletePodRequest>,
    ) -> String {
        info!("删除 Kubernetes Pod: {}", params.name);
        match self.searcher.kubernetes() {
            Some(kube) => match kube
                .delete_pod(&params.name, params.namespace.as_deref())
                .await
            {
                Ok(result) => result,
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Kubernetes client not configured".to_string(),
        }
    }

    #[tool(description = "列出 Kubernetes 命名空间")]
    pub async fn kube_list_namespaces(
        &self,
        _params: Parameters<KubeListNamespacesRequest>,
    ) -> String {
        info!("列出 Kubernetes 命名空间");
        match self.searcher.kubernetes() {
            Some(kube) => match kube.list_namespaces().await {
                Ok(namespaces) => serde_json::to_string(&namespaces)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Kubernetes client not configured".to_string(),
        }
    }

    #[tool(description = "列出 Kubernetes 事件")]
    pub async fn kube_list_events(
        &self,
        Parameters(params): Parameters<KubeListEventsRequest>,
    ) -> String {
        info!("列出 Kubernetes 事件: namespace={:?}", params.namespace);
        match self.searcher.kubernetes() {
            Some(kube) => match kube.list_events(params.namespace.as_deref()).await {
                Ok(events) => serde_json::to_string(&events)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Kubernetes client not configured".to_string(),
        }
    }

    #[tool(description = "列出所有可用的 Kubernetes contexts")]
    pub async fn kube_list_contexts(&self, _params: Parameters<KubeListContextsRequest>) -> String {
        info!("列出 Kubernetes contexts");
        match self.searcher.kubernetes() {
            Some(kube) => match kube.list_contexts().await {
                Ok(contexts) => serde_json::to_string(&contexts)
                    .unwrap_or_else(|_| "Failed to serialize".to_string()),
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Kubernetes client not configured".to_string(),
        }
    }

    #[tool(description = "获取当前 kubeconfig 内容")]
    pub async fn kube_get_config(
        &self,
        Parameters(params): Parameters<KubeGetConfigRequest>,
    ) -> String {
        info!("获取 Kubernetes 配置: minified={:?}", params.minified);
        match self.searcher.kubernetes() {
            Some(kube) => match kube.get_config(params.minified.unwrap_or(false)).await {
                Ok(config) => config,
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: Kubernetes client not configured".to_string(),
        }
    }

    // ========== 企业微信机器人工具 ==========

    #[tool(description = "发送企业微信文本消息")]
    pub async fn weixin_send_text(
        &self,
        Parameters(params): Parameters<WeixinSendTextRequest>,
    ) -> String {
        info!("发送企业微信文本消息: {}", params.content);
        match self.searcher.weixin() {
            Some(weixin) => match weixin.send_text(
                &params.content,
                params.mentioned_list,
                params.mentioned_mobile_list,
            ).await {
                Ok(result) => result,
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: WeChat client not configured. Please set WEIXIN_WEBHOOK_URL environment variable.".to_string(),
        }
    }

    #[tool(description = "发送企业微信 Markdown 消息")]
    pub async fn weixin_send_markdown(
        &self,
        Parameters(params): Parameters<WeixinSendMarkdownRequest>,
    ) -> String {
        info!("发送企业微信 Markdown 消息");
        match self.searcher.weixin() {
            Some(weixin) => match weixin.send_markdown(&params.content).await {
                Ok(result) => result,
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: WeChat client not configured. Please set WEIXIN_WEBHOOK_URL environment variable.".to_string(),
        }
    }

    #[tool(description = "发送企业微信图片消息")]
    pub async fn weixin_send_image(
        &self,
        Parameters(params): Parameters<WeixinSendImageRequest>,
    ) -> String {
        info!("发送企业微信图片消息: media_id={}", params.media_id);
        match self.searcher.weixin() {
            Some(weixin) => match weixin.send_image(&params.media_id).await {
                Ok(result) => result,
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: WeChat client not configured. Please set WEIXIN_WEBHOOK_URL environment variable.".to_string(),
        }
    }

    #[tool(description = "发送企业微信文件消息")]
    pub async fn weixin_send_file(
        &self,
        Parameters(params): Parameters<WeixinSendFileRequest>,
    ) -> String {
        info!("发送企业微信文件消息: media_id={}", params.media_id);
        match self.searcher.weixin() {
            Some(weixin) => match weixin.send_file(&params.media_id).await {
                Ok(result) => result,
                Err(e) => format!("Error: {}", e),
            },
            None => "Error: WeChat client not configured. Please set WEIXIN_WEBHOOK_URL environment variable.".to_string(),
        }
    }

    #[tool(description = "发送企业微信图文消息")]
    pub async fn weixin_send_news(
        &self,
        Parameters(params): Parameters<WeixinSendNewsRequest>,
    ) -> String {
        info!("发送企业微信图文消息: {} 篇文章", params.articles.len());
        match self.searcher.weixin() {
            Some(weixin) => {
                let articles = params.articles.iter().map(|a| crate::searcher::weixin::Article {
                    title: a.title.clone(),
                    description: a.description.clone(),
                    url: a.url.clone(),
                    picurl: a.picurl.clone(),
                }).collect();
                match weixin.send_news(articles).await {
                    Ok(result) => result,
                    Err(e) => format!("Error: {}", e),
                }
            },
            None => "Error: WeChat client not configured. Please set WEIXIN_WEBHOOK_URL environment variable.".to_string(),
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
                "Observability MCP Server providing Prometheus, Loki metrics search, Harbor container registry management, Nacos service discovery and configuration management, Kafka messaging, Doris database operations, Jira, Confluence and Bitbucket Server collaboration tools, WeChat Work (企业微信) webhook notifications, and Kubernetes cluster management!",
            )
            .with_server_info(Implementation::new("observability-mcp-server", "0.5.0"))
    }

    async fn ping(&self, _ctx: RequestContext<RoleServer>) -> Result<(), ErrorData> {
        info!("Received ping request");
        Ok(())
    }

    async fn on_initialized(&self, _ctx: NotificationContext<RoleServer>) {
        info!("Client initialized successfully");
    }
}
