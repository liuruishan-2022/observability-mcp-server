---
title: Alerting with Prometheus
---

# Alerting

Prometheus Alerting allows you to define alert conditions based on PromQL expressions and send notifications when those conditions are triggered.

## Alertmanager

Prometheus uses Alertmanager for handling alerts. It handles:
- Deduplication
- Grouping
- Routing
- Inhibition
- Silencing

## Setting Up Alerting

### 1. Define Alerting Rules

Create a file `alerts.yml`:

```yaml
groups:
  - name: critical_alerts
    interval: 30s
    rules:
      - alert: HighRequestLatency
        expr: histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m])) > 0.5
        for: 10m
        labels:
          severity: critical
        annotations:
          summary: "High latency on {{ $labels.instance }}"
          description: "{{ $labels.instance }} has high latency"

      - alert: ServiceDown
        expr: up{job="my_service"} == 0
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Service {{ $labels.instance }} is down"
```

### 2. Configure Alertmanager

Create `alertmanager.yml`:

```yaml
global:
  resolve_timeout: 5m

route:
  group_by: ['alertname', 'cluster', 'service']
  group_wait: 10s
  group_interval: 10s
  repeat_interval: 12h
  receiver: 'default'

  routes:
    - match:
        severity: critical
      receiver: 'critical'

receivers:
  - name: 'default'
    email_configs:
      - to: 'team@example.com'

  - name: 'critical'
    pagerduty_configs:
      - service_key: '<your-key>'
```

### 3. Update Prometheus Configuration

```yaml
# prometheus.yml
alerting:
  alertmanagers:
    - static_configs:
        - targets:
            - 'localhost:9093'

rule_files:
  - "alerts.yml"
```

## Alert States

An alert goes through these states:
1. **Inactive**: Condition is not met
2. **Pending**: Condition is met, but `for` duration hasn't elapsed
3. **Firing**: Condition is met and `for` duration has elapsed

## Notification Templates

Alertmanager supports Go templating for notifications:

```yaml
annotations:
  summary: "Alert {{ $labels.alertname }}"
  description: "{{ $labels.instance }} is {{ $labels.state }}"
```

Available variables:
- `$labels`: Label set
- `$value`: Evaluated expression value
- `$externalURL`: URL to the alert

## Testing Alerts

Use the Alertmanager API to test:

```bash
# Send a test alert
curl -XPOST http://localhost:9093/api/v1/alerts -d '[{
  "labels": {
    "alertname": "TestAlert",
    "severity": "warning"
  },
  "annotations": {
    "description": "This is a test alert"
  }
}]'
```

## Best Practices

1. **Use meaningful alert names**: Clear and descriptive
2. **Set appropriate `for` duration**: Avoid alert flapping
3. **Document alerts**: Explain what to do when alert fires
4. **Test before deploying**: Validate alert expressions
5. **Group intelligently**: Related alerts should group together

## Common Alert Patterns

### High CPU Usage

```yaml
- alert: HighCPUUsage
  expr: 100 - (avg by (instance) (rate(process_cpu_seconds_total{mode="idle"}[5m])) * 100) > 80
  for: 10m
```

### Disk Space Low

```yaml
- alert: DiskSpaceLow
  expr: (node_filesystem_avail_bytes{mountpoint="/"} / node_filesystem_size_bytes{mountpoint="/"}) * 100 < 10
  for: 5m
```

### Service Instance Down

```yaml
- alert: ServiceDown
  expr: up{job="my_service"} == 0
  for: 2m
```
