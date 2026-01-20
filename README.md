# Observability MCP Server

A comprehensive Model Context Protocol (MCP) server for observability and infrastructure management. Built with Rust for high performance and reliability.

## Supported Systems

- **Prometheus** - Metrics querying and alerting
- **Loki** - Log aggregation and analysis (coming soon)
- **Nacos** - Service discovery and configuration management
- **MySQL** - Database metrics and inspection
- **Doris** - Data warehouse monitoring
- **Kubernetes** - Cluster orchestration and monitoring
- **RocketMQ** - Message queue metrics (coming soon)
- **Kafka** - Stream processing metrics (coming soon)

## Features

- 🚀 High-performance Rust implementation
- 🔍 Unified MCP interface for all systems
- 📊 Real-time metrics and logs
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
PROMETHEUS_ROOT=http://your-prometheus:9090
LOKI_ROOT=http://your-loki:3100
# ... more configurations
```

## MCP Tools

### Prometheus Tools
- `query_instant` - Instant query
- `query_range` - Range query
- `series_metadata` - Series metadata
- `label_values` - Label values
- `alertmanagers` - Alertmanager endpoints
- `docs_list` - List Prometheus documentation
- `docs_read` - Read documentation
- `docs_search` - Search documentation

## Project Structure

```
observability-mcp-server/
├── src/
│   ├── searcher/          # Query implementations
│   │   ├── prometheus.rs  # Prometheus client
│   │   ├── loki.rs        # Loki client (TODO)
│   │   └── mod.rs
│   ├── mcp/
│   │   └── tools.rs       # MCP tool definitions
│   ├── docs/              # Documentation integration
│   └── main.rs
└── Cargo.toml
```

## Roadmap

- [x] Prometheus integration
- [x] Documentation tools
- [ ] Loki integration
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
