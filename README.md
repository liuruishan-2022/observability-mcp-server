# Observability MCP Server

A comprehensive Model Context Protocol (MCP) server for observability, infrastructure management, and container registry operations. Built with Rust for high performance and reliability.

## Supported Systems

- **Prometheus** - Metrics querying and alerting
- **Loki** - Log aggregation and analysis
- **Harbor** - Container registry management
- **Nacos** - Service discovery and configuration management
- **Kafka** - Distributed messaging system

## Features

- 🚀 High-performance Rust implementation
- 🔍 Unified MCP interface for all systems
- 📊 Real-time metrics and logs
- 🐳 Container registry management (Harbor)
- 🔧 Easy configuration via environment variables
- 📚 Official documentation integration
- 🔄 HTTP transport with SSE support

## Quick Start

```bash
# Build
cargo build --release

# Run
./target/release/observability-mcp-server
```

Server will start on `http://localhost:3013/mcp`

## Configuration

Set environment variables in `.env`:

```bash
# Required
PROMETHEUS_ROOT=http://your-prometheus:9090
LOKI_ROOT=http://your-loki:3100

# Optional - Harbor (for container registry management)
HARBOR_URL=https://harbor.example.com
HARBOR_USERNAME=admin
HARBOR_PASSWORD=Harbor12345

# Optional - Nacos (for service discovery and configuration management, requires Nacos 3.0+)
NACOS_URL=http://your-nacos:8848
NACOS_ACCESS_TOKEN=your_access_token

# Optional - Kafka (for messaging)
KAFKA_BOOTSTRAP_SERVERS=localhost:9092
KAFKA_CONSUMER_GROUP_ID=mcp-kafka-consumer-group
KAFKA_USERNAME=your_kafka_username
KAFKA_PASSWORD=your_kafka_password
KAFKA_SECURITY_PROTOCOL=plaintext
```

## MCP Tools

### Prometheus Tools

#### Query Tools
- `prom_query` - Execute PromQL instant query
- `prom_query_range` - Execute PromQL range query
- `prom_series` - Query time series matching selectors
- `prom_query_exemplars` - Query exemplars matching a query

#### Metadata Tools
- `prom_labels` - Get all label names
- `prom_label_values` - Get values for a specific label
- `prom_metadata` - Get metrics metadata
- `prom_targets_metadata` - Get targets metadata

#### Management Tools
- `prom_config` - Get Prometheus configuration
- `prom_reload` - Reload Prometheus configuration
- `prom_delete_series` - Delete time series data
- `prom_snapshot` - Create TSDB snapshot
- `prom_clean_tombstones` - Clean TSDB tombstones
- `prom_quit` - Shutdown Prometheus gracefully

#### System Tools
- `prom_build_info` - Get Prometheus build information
- `prom_healthy` - Health check
- `prom_ready` - Readiness check
- `prom_flags` - Get command-line flags
- `prom_runtime_info` - Get runtime information
- `prom_tsdb_status` - Get TSDB status
- `prom_wal_replay` - Get WAL replay status

#### Alerting Tools
- `prom_alerts` - Get active alerts
- `prom_alert_managers` - Get AlertManager endpoints
- `prom_rules` - Get recording and alerting rules
- `prom_targets` - Get all targets information

### Loki Tools
- `loki_query` - Execute LogQL query
- `loki_labels` - Get all label names from Loki
- `loki_label_values` - Get values for a specific label

### Harbor Tools

#### Project Management
- `harbor_get_projects` - List all projects in Harbor
- `harbor_get_project` - Get specific project information
- `harbor_create_project` - Create a new project
- `harbor_delete_project` - Delete a project

#### Repository Management
- `harbor_get_repositories` - List repositories in a project
- `harbor_delete_repository` - Delete a repository

#### Artifact Management
- `harbor_get_artifacts` - List artifacts (image tags) in a repository
- `harbor_delete_artifact` - Delete an artifact (image tag)

#### Helm Chart Management
- `harbor_get_helm_charts` - List Helm charts in a project
- `harbor_get_helm_chart_versions` - Get versions of a Helm chart
- `harbor_delete_helm_chart_version` - Delete a specific Helm chart version

### Nacos Tools

#### Namespace Management
- `nacos_list_namespaces` - List all namespaces in Nacos cluster

#### Service Discovery
- `nacos_list_services` - List services under a namespace with pagination
- `nacos_get_service` - Get detailed information of a specific service
- `nacos_list_instances` - List instances of a specific service
- `nacos_list_service_subscribers` - List subscribers of a specific service

#### Configuration Management
- `nacos_list_configs` - List configurations under a namespace with pagination
- `nacos_get_config` - Get details of a specific configuration
- `nacos_list_config_history` - List configuration change history
- `nacos_get_config_history` - Get specific configuration history record
- `nacos_list_config_listeners` - List listeners subscribed to a configuration
- `nacos_list_listened_configs` - List configurations subscribed by a client IP

### Kafka Tools

#### Topic Management
- `kafka_create_topic` - Create a new Kafka topic with configurable partitions and replication factor
- `kafka_list_topics` - List all available Kafka topics in the cluster
- `kafka_delete_topic` - Delete an existing Kafka topic
- `kafka_describe_topic` - Get detailed information about a specific topic including partition details

#### Messaging
- `kafka_produce_message` - Send messages to a Kafka topic with support for message keys and headers
- `kafka_consume_messages` - Read messages from a Kafka topic with configurable timeout

### Documentation Tools
- `docs_list` - List all available Prometheus documentation files
- `docs_read` - Read specific documentation file
- `docs_search` - Search documentation for keywords

## Project Structure

```
observability-mcp-server/
├── src/
│   ├── searcher/          # API client implementations
│   │   ├── prometheus.rs  # Prometheus client
│   │   ├── loki.rs        # Loki client
│   │   ├── harbor.rs      # Harbor client
│   │   ├── nacos.rs       # Nacos client
│   │   ├── kafka.rs       # Kafka client
│   ��   └── mod.rs
│   ├── mcp/
│   │   └── tools.rs       # MCP tool definitions
│   ├── docs/              # Documentation integration
│   └── main.rs
├── docs/                  # Prometheus official documentation
├── Cargo.toml
└── README.md
```

## Usage Examples

### Harbor Project Management

```javascript
// List all projects
const projects = await mcp.callTool("harbor_get_projects", {});

// Create a new project
await mcp.callTool("harbor_create_project", {
  projectName: "my-new-project",
  public: false
});

// Get repositories in a project
const repos = await mcp.callTool("harbor_get_repositories", {
  projectIdOrName: "my-project"
});
```

### Nacos Service Discovery

```javascript
// List all namespaces
const namespaces = await mcp.callTool("nacos_list_namespaces", {});

// List services
const services = await mcp.callTool("nacos_list_services", {
  pageNo: 1,
  pageSize: 100,
  namespaceId: "public"
});

// Get service instances
const instances = await mcp.callTool("nacos_list_instances", {
  serviceName: "my-service",
  namespaceId: "public",
  groupName: "DEFAULT_GROUP"
});
```

### Nacos Configuration Management

```javascript
// List configurations
const configs = await mcp.callTool("nacos_list_configs", {
  pageNo: 1,
  pageSize: 100,
  namespaceId: "public"
});

// Get specific configuration
const config = await mcp.callTool("nacos_get_config", {
  dataId: "application.properties",
  groupName: "DEFAULT_GROUP",
  namespaceId: "public"
});

// List configuration history
const history = await mcp.callTool("nacos_list_config_history", {
  dataId: "application.properties",
  groupName: "DEFAULT_GROUP",
  pageNo: 1,
  pageSize: 10
});
```

### Kafka Messaging

```javascript
// List all topics
const topics = await mcp.callTool("kafka_list_topics", {});

// Create a new topic
await mcp.callTool("kafka_create_topic", {
  topic: "my-new-topic",
  numPartitions: 3,
  replicationFactor: 2
});

// Describe topic details
const topicInfo = await mcp.callTool("kafka_describe_topic", {
  topic: "my-topic"
});

// Produce a message
const produceResult = await mcp.callTool("kafka_produce_message", {
  topic: "my-topic",
  key: "my-key",
  value: "Hello, Kafka!",
  headers: [["content-type", "text/plain"]]
});

// Consume messages
const messages = await mcp.callTool("kafka_consume_messages", {
  topic: "my-topic",
  timeoutSeconds: 10
});

// Delete a topic
await mcp.callTool("kafka_delete_topic", {
  topic: "old-topic"
});
```

### Prometheus Querying

```javascript
// Instant query
const result = await mcp.callTool("prom_query", {
  query: "up{job='prometheus'}"
});

// Range query
const rangeResult = await mcp.callTool("prom_query_range", {
  query: "rate(prometheus_http_requests_total[5m])",
  start: "2025-01-01T00:00:00Z",
  end: "2025-01-01T01:00:00Z",
  step: "1m"
});
```

## Roadmap

- [x] Prometheus integration
- [x] Loki integration
- [x] Harbor integration
- [x] Nacos integration
- [x] Kafka integration
- [x] Documentation tools
- [ ] MySQL integration
- [ ] Doris integration
- [ ] Kubernetes integration
- [ ] RocketMQ integration

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License
