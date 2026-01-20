---
title: Getting Started with Prometheus
---

# Getting Started

Prometheus is an open-source systems monitoring and alerting toolkit. It collects and stores its metrics as time series data, i.e., metrics information is stored with the timestamp at which it was recorded, alongside optional key-value pairs called dimensions.

## Installation

### Using Docker

```bash
docker run -p 9090:9090 prom/prometheus
```

### From Source

```bash
git clone https://github.com/prometheus/prometheus.git
cd prometheus
make build
./prometheus --config.file=your_config.yml
```

## Basic Concepts

### Metrics

Prometheus has four primary metric types:

1. **Counter**: A cumulative metric that represents a single monotonically increasing counter
2. **Gauge**: A metric that represents a single numerical value that can arbitrarily go up and down
3. **Histogram**: A histogram samples observations and counts them in configurable buckets
4. **Summary**: Similar to a histogram, but with configurable quantiles

### PromQL

PromQL (Prometheus Query Language) allows you to query and aggregate time series data.

Example queries:

```promql
# Rate of HTTP requests
rate(http_requests_total[5m])

# CPU usage by instance
rate(process_cpu_seconds_total[5m]) by (instance)

# Memory usage over 1 hour
avg_over_time(process_resident_memory_bytes[1h])
```

## Configuration

The basic configuration file (`prometheus.yml`) looks like:

```yaml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'prometheus'
    static_configs:
      - targets: ['localhost:9090']
```

## Next Steps

- Learn about [Recording Rules](recording_rules.md)
- Configure [Alerting](alerting.md)
- Set up [Visualization](visualization.md)
