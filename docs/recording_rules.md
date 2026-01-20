---
title: Recording Rules
---

# Recording Rules

Recording rules allow you to pre-compute frequently needed or computationally expensive expressions and save their result as a new set of time series.

## Why Use Recording Rules?

Recording rules are useful for:
- Reducing computation load for frequently queried expressions
- Making dashboards faster
- Simplifying complex queries

## Defining Recording Rules

Recording rules are defined in a separate file and loaded into Prometheus:

```yaml
groups:
  - name: api_rules
    interval: 30s
    rules:
      - record: job:http_requests_total:rate1m
        expr: sum by (job) (rate(http_requests_total[1m]))

      - record: job:http_requests_total:rate5m
        expr: sum by (job) (rate(http_requests_total[5m]))
```

## Loading Rules

Add the rule files to your `prometheus.yml`:

```yaml
rule_files:
  - "rules/*.rules"
```

Reload Prometheus:

```bash
kill -HUP <pid>
```

Or use the HTTP API:

```bash
curl -X POST http://localhost:9090/-/reload
```

## Best Practices

1. **Name your rules consistently**: Use a colon (`:`) as separator
2. **Document your rules**: Add comments explaining the purpose
3. **Test before deploying**: Use `promtool check rules`
4. **Organize by purpose**: Group related rules together

## Performance Considerations

- Rules are evaluated at their configured interval
- More rules = more CPU and memory usage
- Use appropriate intervals (not too frequent)

## Examples

### Calculate Request Rate per Endpoint

```yaml
- record: endpoint:http_requests:rate5m
  expr: sum by (endpoint) (rate(http_requests_total{endpoint=~"/api/.*"}[5m]))
```

### Calculate 95th Percentile Response Time

```yaml
- record: endpoint:response_time:p95
  expr: histogram_quantile(0.95, sum by (endpoint, le) (rate(http_request_duration_seconds_bucket[5m])))
```

### Calculate Error Rate

```yaml
- record: endpoint:error_rate:5m
  expr: sum by (endpoint) (rate(http_requests_total{status=~"5.."}[5m])) / sum by (endpoint) (rate(http_requests_total[5m]))
```
