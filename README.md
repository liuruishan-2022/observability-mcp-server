# Observability MCP Server

A comprehensive Model Context Protocol (MCP) server for observability, infrastructure management, and container registry operations. Built with Rust for high performance and reliability.

## Supported Systems

- **Prometheus** - Metrics querying and alerting
- **Loki** - Log aggregation and analysis
- **Harbor** - Container registry management

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
│   │   └── mod.rs
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
- [x] Documentation tools
- [ ] Nacos integration
- [ ] MySQL integration
- [ ] Doris integration
- [ ] Kubernetes integration
- [ ] RocketMQ integration
- [ ] Kafka integration

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License
