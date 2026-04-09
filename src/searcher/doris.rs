//! Doris searcher for Apache Doris database
//!
//! Provides MCP tools for querying Apache Doris database

use super::SearcherError;
use chrono::Timelike;
use serde::{Deserialize, Serialize};
use sqlx::{
    Column, MySql, Row,
    mysql::{MySqlPool, MySqlRow},
    pool::PoolConnection,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::{sync::Mutex, time::sleep};
use uuid::Uuid;

const DEFAULT_DORIS_CATALOG: &str = "internal";
const DEFAULT_QUERY_TIMEOUT_SECS: u64 = 30;
const MAX_QUERY_TIMEOUT_SECS: u64 = 300;
const DEFAULT_QUERY_MAX_ROWS: usize = 100;
const MAX_QUERY_MAX_ROWS: usize = 10_000;
const DEFAULT_AUDIT_LOG_DAYS: u32 = 7;
const MAX_AUDIT_LOG_DAYS: u32 = 365;
const DEFAULT_AUDIT_LOG_LIMIT: usize = 100;
const MAX_AUDIT_LOG_LIMIT: usize = 1_000;
const DEFAULT_ANALYSIS_SAMPLE_SIZE: usize = 100_000;
const MAX_ANALYSIS_SAMPLE_SIZE: usize = 1_000_000;
const DEFAULT_ANALYSIS_AUDIT_LOG_LIMIT: usize = 10_000;

/// Doris client for executing SQL queries
pub struct DorisClient {
    /// MySQL connection pool (Doris uses MySQL protocol)
    /// Wrapped in Arc<Mutex<>> for lazy initialization and thread-safe access
    pool: Arc<Mutex<Option<sqlx::MySqlPool>>>,
    /// Connection components for lazy initialization
    host: String,
    port: u16,
    username: String,
    password: String,
    database: String,
    default_catalog: String,
    /// HTTP API URL for metadata
    http_url: Option<String>,
    /// Optional reqwest client for HTTP API calls
    http_client: Option<reqwest::Client>,
}

impl DorisClient {
    /// Create a new Doris client (connection is established lazily)
    ///
    /// # Arguments
    /// * `host` - Doris host address
    /// * `port` - Doris MySQL protocol port (default 9030)
    /// * `username` - Doris username
    /// * `password` - Doris password
    /// * `database` - Default database name
    /// * `http_url` - Optional HTTP API URL (e.g., http://host:8030)
    pub fn new(
        host: String,
        port: String,
        username: String,
        password: String,
        database: String,
        http_url: Option<String>,
    ) -> Self {
        let port = port.parse::<u16>().unwrap_or(9030);
        tracing::info!("Creating Doris client with {}:{}", host, port);

        DorisClient {
            pool: Arc::new(Mutex::new(None)),
            host,
            port,
            username,
            password,
            database,
            default_catalog: DEFAULT_DORIS_CATALOG.to_string(),
            http_url: http_url.clone(),
            http_client: http_url.is_some().then(reqwest::Client::new),
        }
    }

    /// Ensure the database connection is established
    async fn ensure_connected(&self) -> Result<(), SearcherError> {
        let mut pool_guard = self.pool.lock().await;
        if pool_guard.is_none() {
            tracing::info!("Establishing Doris database connection...");

            let options = sqlx::mysql::MySqlConnectOptions::new()
                .host(&self.host)
                .port(self.port)
                .username(&self.username)
                .password(&self.password)
                .database(&self.database)
                .no_engine_substitution(false)
                .pipes_as_concat(false);

            let pool = sqlx::mysql::MySqlPoolOptions::new()
                .max_connections(5)
                .acquire_timeout(std::time::Duration::from_secs(30))
                .connect_with(options)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to connect to Doris: {}", e);
                    SearcherError::ApiError(format!("Failed to connect to Doris: {}", e))
                })?;

            tracing::info!("Successfully connected to Doris");
            *pool_guard = Some(pool);
        }
        Ok(())
    }

    /// Get a clone of the pool, initializing if necessary
    async fn get_pool(&self) -> Result<MySqlPool, SearcherError> {
        self.ensure_connected().await?;
        let pool_guard = self.pool.lock().await;
        pool_guard
            .as_ref()
            .cloned()
            .ok_or_else(|| SearcherError::ApiError("Doris connection pool not initialized".to_string()))
    }

    async fn acquire_connection(&self) -> Result<PoolConnection<MySql>, SearcherError> {
        let pool = self.get_pool().await?;
        pool.acquire()
            .await
            .map_err(|e| SearcherError::ApiError(format!("Failed to acquire Doris connection: {}", e)))
    }

    fn effective_database_name<'a>(&'a self, db_name: Option<&'a str>) -> &'a str {
        db_name.unwrap_or(self.database.as_str())
    }

    fn effective_catalog_name<'a>(&'a self, catalog_name: Option<&'a str>) -> &'a str {
        catalog_name.unwrap_or(self.default_catalog.as_str())
    }

    fn normalize_timeout_secs(timeout_secs: Option<u64>) -> u64 {
        timeout_secs
            .unwrap_or(DEFAULT_QUERY_TIMEOUT_SECS)
            .clamp(1, MAX_QUERY_TIMEOUT_SECS)
    }

    fn normalize_max_rows(max_rows: Option<usize>) -> usize {
        max_rows
            .unwrap_or(DEFAULT_QUERY_MAX_ROWS)
            .clamp(1, MAX_QUERY_MAX_ROWS)
    }

    fn normalize_audit_log_days(days: Option<u32>) -> u32 {
        days.unwrap_or(DEFAULT_AUDIT_LOG_DAYS)
            .clamp(1, MAX_AUDIT_LOG_DAYS)
    }

    fn normalize_audit_log_limit(limit: Option<usize>) -> usize {
        limit
            .unwrap_or(DEFAULT_AUDIT_LOG_LIMIT)
            .clamp(1, MAX_AUDIT_LOG_LIMIT)
    }

    fn normalize_analysis_sample_size(sample_size: Option<usize>) -> usize {
        sample_size
            .unwrap_or(DEFAULT_ANALYSIS_SAMPLE_SIZE)
            .clamp(1, MAX_ANALYSIS_SAMPLE_SIZE)
    }

    fn validate_identifier(identifier: &str, identifier_type: &str) -> Result<(), SearcherError> {
        let trimmed = identifier.trim();
        if trimmed.is_empty() {
            return Err(SearcherError::ApiError(format!(
                "{} cannot be empty",
                identifier_type
            )));
        }

        if !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(SearcherError::ApiError(format!(
                "Invalid {}: {}",
                identifier_type, identifier
            )));
        }

        Ok(())
    }

    fn quote_identifier(identifier: &str) -> String {
        format!("`{}`", identifier.replace('`', "``"))
    }

    fn escape_sql_string(value: &str) -> String {
        value.replace('\\', "\\\\").replace('\'', "''")
    }

    fn table_reference(db_name: &str, table_name: &str) -> String {
        format!(
            "{}.{}",
            Self::quote_identifier(db_name),
            Self::quote_identifier(table_name)
        )
    }

    fn qualified_table_name(catalog_name: &str, db_name: &str, table_name: &str) -> String {
        format!("{}.{}.{}", catalog_name, db_name, table_name)
    }

    fn json_value_to_u64(value: &serde_json::Value) -> Option<u64> {
        match value {
            serde_json::Value::Number(number) => number
                .as_u64()
                .or_else(|| number.as_i64().and_then(|v| (v >= 0).then_some(v as u64))),
            serde_json::Value::String(text) => text.parse::<u64>().ok(),
            _ => None,
        }
    }

    fn json_value_to_bool(value: &serde_json::Value) -> Option<bool> {
        match value {
            serde_json::Value::Bool(value) => Some(*value),
            serde_json::Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" => Some(true),
                "false" | "0" | "no" => Some(false),
                _ => None,
            },
            serde_json::Value::Number(number) => number.as_i64().map(|value| value != 0),
            _ => None,
        }
    }

    fn json_value_to_string(value: &serde_json::Value) -> Option<String> {
        match value {
            serde_json::Value::String(text) => Some(text.to_string()),
            serde_json::Value::Number(number) => Some(number.to_string()),
            serde_json::Value::Bool(value) => Some(value.to_string()),
            _ => None,
        }
    }

    fn normalize_choice(
        value: Option<&str>,
        default: &str,
        allowed: &[&str],
        field_name: &str,
    ) -> Result<String, SearcherError> {
        let normalized = value.unwrap_or(default).trim().to_ascii_lowercase();
        if allowed.iter().any(|item| *item == normalized) {
            Ok(normalized)
        } else {
            Err(SearcherError::ApiError(format!(
                "Invalid {}: {}. Allowed values: {}",
                field_name,
                normalized,
                allowed.join(", ")
            )))
        }
    }

    fn format_bytes(bytes_value: u64) -> String {
        if bytes_value == 0 {
            return "0 B".to_string();
        }

        let units = ["B", "KB", "MB", "GB", "TB", "PB"];
        let mut unit_index = 0usize;
        let mut size = bytes_value as f64;

        while size >= 1024.0 && unit_index < units.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", bytes_value, units[unit_index])
        } else {
            format!("{size:.2} {}", units[unit_index])
        }
    }

    fn response_preview(text: &str, max_chars: usize) -> String {
        text.chars().take(max_chars).collect()
    }

    fn to_json_value<T: Serialize>(value: &T) -> serde_json::Value {
        serde_json::to_value(value).unwrap_or(serde_json::Value::Null)
    }

    fn parse_prometheus_labels(input: &str) -> BTreeMap<String, String> {
        let chars: Vec<char> = input.chars().collect();
        let mut labels = BTreeMap::new();
        let mut index = 0usize;

        while index < chars.len() {
            while index < chars.len() && (chars[index] == ',' || chars[index].is_whitespace()) {
                index += 1;
            }
            if index >= chars.len() {
                break;
            }

            let key_start = index;
            while index < chars.len() && chars[index] != '=' {
                index += 1;
            }
            if index >= chars.len() {
                break;
            }

            let key: String = chars[key_start..index].iter().collect();
            index += 1;

            let mut value = String::new();
            if index < chars.len() && chars[index] == '"' {
                index += 1;
                let mut escaped = false;
                while index < chars.len() {
                    let current = chars[index];
                    if escaped {
                        value.push(match current {
                            'n' => '\n',
                            'r' => '\r',
                            't' => '\t',
                            '\\' => '\\',
                            '"' => '"',
                            other => other,
                        });
                        escaped = false;
                    } else if current == '\\' {
                        escaped = true;
                    } else if current == '"' {
                        index += 1;
                        break;
                    } else {
                        value.push(current);
                    }
                    index += 1;
                }
            } else {
                let value_start = index;
                while index < chars.len() && chars[index] != ',' {
                    index += 1;
                }
                value = chars[value_start..index]
                    .iter()
                    .collect::<String>()
                    .trim()
                    .to_string();
            }

            labels.insert(key.trim().to_string(), value);

            while index < chars.len() && chars[index] != ',' {
                index += 1;
            }
            if index < chars.len() && chars[index] == ',' {
                index += 1;
            }
        }

        labels
    }

    fn parse_prometheus_metrics(
        metrics_text: &str,
    ) -> BTreeMap<String, MonitoringMetricValue> {
        let mut metrics = BTreeMap::new();

        for line in metrics_text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let mut parts = line.split_whitespace();
            let Some(metric_part) = parts.next() else {
                continue;
            };
            let Some(value_part) = parts.next() else {
                continue;
            };
            let Ok(value) = value_part.parse::<f64>() else {
                continue;
            };

            if let Some(label_start) = metric_part.find('{') {
                let Some(label_end) = metric_part.rfind('}') else {
                    continue;
                };
                let metric_name = &metric_part[..label_start];
                let labels = Self::parse_prometheus_labels(&metric_part[label_start + 1..label_end]);
                let sample = MonitoringMetricSample { labels, value };

                match metrics.remove(metric_name) {
                    Some(MonitoringMetricValue::Number(previous_value)) => {
                        metrics.insert(
                            metric_name.to_string(),
                            MonitoringMetricValue::Samples(vec![
                                MonitoringMetricSample {
                                    labels: BTreeMap::new(),
                                    value: previous_value,
                                },
                                sample,
                            ]),
                        );
                    }
                    Some(MonitoringMetricValue::Samples(mut samples)) => {
                        samples.push(sample);
                        metrics.insert(
                            metric_name.to_string(),
                            MonitoringMetricValue::Samples(samples),
                        );
                    }
                    None => {
                        metrics.insert(
                            metric_name.to_string(),
                            MonitoringMetricValue::Samples(vec![sample]),
                        );
                    }
                }
            } else {
                metrics.insert(
                    metric_part.to_string(),
                    MonitoringMetricValue::Number(value),
                );
            }
        }

        metrics
    }

    fn normalized_sql_token(token: &str) -> String {
        token
            .trim_matches(|c: char| {
                c.is_whitespace()
                    || matches!(c, ',' | ';' | '(' | ')' | '[' | ']' | '{' | '}' | '\n' | '\r')
            })
            .trim_matches('`')
            .trim_matches('\'')
            .trim_matches('"')
            .to_string()
    }

    fn canonicalize_table_reference(
        token: &str,
        catalog_name: Option<&str>,
        db_name: Option<&str>,
    ) -> Option<String> {
        let cleaned = Self::normalized_sql_token(token);
        if cleaned.is_empty() {
            return None;
        }

        let lowered = cleaned.to_ascii_lowercase();
        if matches!(
            lowered.as_str(),
            "select"
                | "from"
                | "join"
                | "where"
                | "group"
                | "order"
                | "having"
                | "limit"
                | "union"
                | "on"
                | "left"
                | "right"
                | "inner"
                | "outer"
                | "cross"
                | "full"
                | "with"
                | "as"
                | "and"
                | "or"
        ) {
            return None;
        }

        let parts: Vec<&str> = cleaned.split('.').filter(|part| !part.is_empty()).collect();
        match parts.len() {
            1 => db_name
                .map(|database| {
                    Self::qualified_table_name(
                        catalog_name.unwrap_or(DEFAULT_DORIS_CATALOG),
                        database,
                        parts[0],
                    )
                })
                .or_else(|| {
                    catalog_name.map(|catalog| format!("{}.{}", catalog, parts[0]))
                })
                .or_else(|| Some(parts[0].to_string())),
            2 => catalog_name
                .map(|catalog| Self::qualified_table_name(catalog, parts[0], parts[1]))
                .or_else(|| Some(format!("{}.{}", parts[0], parts[1]))),
            3 => Some(Self::qualified_table_name(parts[0], parts[1], parts[2])),
            _ => None,
        }
    }

    fn extract_table_references_from_sql(
        sql: &str,
        catalog_name: Option<&str>,
        db_name: Option<&str>,
    ) -> Vec<String> {
        let lowered = sql.to_ascii_lowercase();
        let tokens = lowered
            .split_whitespace()
            .zip(sql.split_whitespace())
            .collect::<Vec<_>>();
        let mut references = Vec::new();
        let mut index = 0usize;

        while index < tokens.len() {
            let token = tokens[index].0;
            if matches!(token, "from" | "join" | "into" | "update") {
                if let Some((_, original)) = tokens.get(index + 1) {
                    if let Some(reference) =
                        Self::canonicalize_table_reference(original, catalog_name, db_name)
                    {
                        references.push(reference);
                    }
                }
            }
            index += 1;
        }

        references.sort();
        references.dedup();
        references
    }

    fn extract_destination_table_from_sql(
        sql: &str,
        catalog_name: Option<&str>,
        db_name: Option<&str>,
    ) -> Option<String> {
        let lowered = sql.to_ascii_lowercase();
        let tokens = lowered
            .split_whitespace()
            .zip(sql.split_whitespace())
            .collect::<Vec<_>>();

        for index in 0..tokens.len() {
            match tokens[index].0 {
                "into" | "update" => {
                    if let Some((_, original)) = tokens.get(index + 1) {
                        return Self::canonicalize_table_reference(original, catalog_name, db_name);
                    }
                }
                "table" => {
                    if index > 0
                        && matches!(tokens[index - 1].0, "create" | "replace" | "alter")
                    {
                        if let Some((_, original)) = tokens.get(index + 1) {
                            return Self::canonicalize_table_reference(
                                original,
                                catalog_name,
                                db_name,
                            );
                        }
                    }
                }
                "view" => {
                    if index > 0 && matches!(tokens[index - 1].0, "create" | "replace" | "alter") {
                        if let Some((_, original)) = tokens.get(index + 1) {
                            return Self::canonicalize_table_reference(
                                original,
                                catalog_name,
                                db_name,
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        None
    }

    fn extract_select_transformations(sql: &str, target_column: &str) -> Vec<String> {
        let lowered = sql.to_ascii_lowercase();
        let target = target_column.to_ascii_lowercase();
        let Some(select_index) = lowered.find("select") else {
            return Vec::new();
        };
        let from_index = lowered[select_index..]
            .find(" from ")
            .map(|offset| select_index + offset)
            .unwrap_or(sql.len());
        let select_clause = &sql[select_index + "select".len()..from_index];

        select_clause
            .split(',')
            .filter_map(|part| {
                let part_trimmed = part.trim();
                let lowered_part = part_trimmed.to_ascii_lowercase();
                if lowered_part.contains(&format!(" as {}", target))
                    || lowered_part.ends_with(&format!(" {}", target))
                    || lowered_part == target
                {
                    Some(part_trimmed.to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    fn simplify_sql_pattern(sql: &str) -> String {
        let mut simplified = String::new();
        let mut in_single_quote = false;
        let mut previous_was_space = false;

        for ch in sql.chars() {
            if ch == '\'' {
                let was_in_single_quote = in_single_quote;
                in_single_quote = !in_single_quote;
                if !was_in_single_quote && !simplified.ends_with('?') {
                    simplified.push('?');
                    previous_was_space = false;
                }
                continue;
            }

            if in_single_quote {
                continue;
            }

            if ch.is_ascii_digit() {
                if !simplified.ends_with('?') {
                    simplified.push('?');
                }
                previous_was_space = false;
                continue;
            }

            if ch.is_whitespace() {
                if !previous_was_space {
                    simplified.push(' ');
                    previous_was_space = true;
                }
                continue;
            }

            previous_was_space = false;
            simplified.push(ch.to_ascii_uppercase());
        }

        simplified.trim().to_string()
    }

    fn classify_sql_statement(sql: &str) -> String {
        sql.split_whitespace()
            .next()
            .map(|token| token.to_ascii_uppercase())
            .unwrap_or_else(|| "UNKNOWN".to_string())
    }

    fn parse_datetime_string(value: &str) -> Option<chrono::NaiveDateTime> {
        [
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d %H:%M:%S%.f",
            "%Y-%m-%dT%H:%M:%S",
            "%Y-%m-%dT%H:%M:%S%.f",
        ]
        .iter()
        .find_map(|format| chrono::NaiveDateTime::parse_from_str(value, format).ok())
    }

    fn parse_distribution_from_ddl(ddl: &str) -> serde_json::Value {
        let lowered = ddl.to_ascii_lowercase();
        let mut distribution_type = None;
        let mut distribution_columns: Vec<String> = Vec::new();
        let mut bucket_num = None;

        if let Some(index) = lowered.find("distributed by hash(") {
            distribution_type = Some("HASH".to_string());
            let start = index + "distributed by hash(".len();
            if let Some(end_offset) = lowered[start..].find(')') {
                let columns = &ddl[start..start + end_offset];
                distribution_columns = columns
                    .split(',')
                    .map(|value| value.trim().trim_matches('`').to_string())
                    .filter(|value| !value.is_empty())
                    .collect();
            }
        } else if lowered.contains("distributed by random") {
            distribution_type = Some("RANDOM".to_string());
        }

        if let Some(index) = lowered.find(" buckets ") {
            let bucket_segment = &ddl[index + " buckets ".len()..];
            let bucket_token = bucket_segment
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim_matches(|c: char| c == ';' || c == ',');
            bucket_num = Some(bucket_token.to_string());
        }

        serde_json::json!({
            "distribution_type": distribution_type,
            "distribution_columns": distribution_columns,
            "bucket_num": bucket_num
        })
    }

    fn parse_partitioning_from_ddl(ddl: &str) -> serde_json::Value {
        let lowered = ddl.to_ascii_lowercase();
        let partition_type = if lowered.contains("partition by range") {
            Some("RANGE".to_string())
        } else if lowered.contains("partition by list") {
            Some("LIST".to_string())
        } else if lowered.contains("partition by") {
            Some("OTHER".to_string())
        } else {
            None
        };

        let key_type = ["unique key", "aggregate key", "duplicate key", "primary key"]
            .iter()
            .find_map(|key_type| lowered.contains(key_type).then_some((*key_type).to_ascii_uppercase()));

        serde_json::json!({
            "partition_type": partition_type,
            "key_type": key_type
        })
    }

    async fn check_tcp_endpoint(host: &str, port: u16, timeout_secs: u64) -> bool {
        tokio::time::timeout(
            Duration::from_secs(timeout_secs.max(1)),
            tokio::net::TcpStream::connect((host, port)),
        )
        .await
        .map(|result| result.is_ok())
        .unwrap_or(false)
    }

    fn is_numeric_data_type(data_type: &str) -> bool {
        matches!(
            data_type.to_ascii_lowercase().as_str(),
            "tinyint"
                | "smallint"
                | "int"
                | "integer"
                | "bigint"
                | "float"
                | "double"
                | "decimal"
                | "numeric"
                | "real"
        )
    }

    fn is_temporal_data_type(data_type: &str) -> bool {
        matches!(
            data_type.to_ascii_lowercase().as_str(),
            "date" | "datetime" | "timestamp" | "time"
        )
    }

    fn is_categorical_data_type(data_type: &str) -> bool {
        !Self::is_numeric_data_type(data_type) && !Self::is_temporal_data_type(data_type)
    }

    fn upsert_table_data_size_entry(
        report: &mut TableDataSizeReport,
        database_name: &str,
        table_name: &str,
        size_bytes: u64,
        replica_count: Option<u64>,
        details: serde_json::Value,
    ) {
        let db_entry = report
            .databases
            .entry(database_name.to_string())
            .or_insert_with(|| TableDataSizeDatabase {
                database_name: database_name.to_string(),
                table_count: 0,
                total_size_bytes: 0,
                total_size_formatted: "0 B".to_string(),
                tables: BTreeMap::new(),
            });

        if let Some(previous) = db_entry.tables.insert(
            table_name.to_string(),
            TableDataSizeTable {
                table_name: table_name.to_string(),
                size_bytes,
                size_formatted: Self::format_bytes(size_bytes),
                replica_count,
                details,
            },
        ) {
            db_entry.total_size_bytes = db_entry
                .total_size_bytes
                .saturating_sub(previous.size_bytes);
            report.summary.total_size_bytes = report
                .summary
                .total_size_bytes
                .saturating_sub(previous.size_bytes);
        }

        db_entry.total_size_bytes += size_bytes;
        report.summary.total_size_bytes += size_bytes;
    }

    fn process_table_data_size_record(
        report: &mut TableDataSizeReport,
        record: &serde_json::Value,
        default_db_name: Option<&str>,
        default_table_name: Option<&str>,
    ) {
        let Some(object) = record.as_object() else {
            return;
        };

        let database_name = object
            .get("database")
            .or_else(|| object.get("db"))
            .and_then(|value| value.as_str())
            .or(default_db_name)
            .unwrap_or("unknown");
        let table_name = object
            .get("table")
            .or_else(|| object.get("table_name"))
            .and_then(|value| value.as_str())
            .or(default_table_name)
            .unwrap_or("unknown");
        let size_bytes = object
            .get("size")
            .or_else(|| object.get("size_bytes"))
            .or_else(|| object.get("data_size"))
            .and_then(Self::json_value_to_u64)
            .unwrap_or(0);
        let replica_count = object
            .get("replica_count")
            .or_else(|| object.get("replicaNum"))
            .and_then(Self::json_value_to_u64);

        Self::upsert_table_data_size_entry(
            report,
            database_name,
            table_name,
            size_bytes,
            replica_count,
            record.clone(),
        );
    }

    fn process_table_data_size_tables(
        report: &mut TableDataSizeReport,
        database_name: &str,
        tables: &serde_json::Value,
        default_table_name: Option<&str>,
    ) {
        match tables {
            serde_json::Value::Object(entries) => {
                for (table_name, table_info) in entries {
                    let size_bytes = table_info
                        .get("size")
                        .or_else(|| table_info.get("size_bytes"))
                        .or_else(|| table_info.get("data_size"))
                        .and_then(Self::json_value_to_u64)
                        .unwrap_or(0);
                    let replica_count = table_info
                        .get("replica_count")
                        .or_else(|| table_info.get("replicaNum"))
                        .and_then(Self::json_value_to_u64);

                    Self::upsert_table_data_size_entry(
                        report,
                        database_name,
                        table_name,
                        size_bytes,
                        replica_count,
                        table_info.clone(),
                    );
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    let table_name = item
                        .get("table")
                        .or_else(|| item.get("table_name"))
                        .and_then(|value| value.as_str())
                        .or(default_table_name)
                        .unwrap_or("unknown");
                    let size_bytes = item
                        .get("size")
                        .or_else(|| item.get("size_bytes"))
                        .or_else(|| item.get("data_size"))
                        .and_then(Self::json_value_to_u64)
                        .unwrap_or(0);
                    let replica_count = item
                        .get("replica_count")
                        .or_else(|| item.get("replicaNum"))
                        .and_then(Self::json_value_to_u64);

                    Self::upsert_table_data_size_entry(
                        report,
                        database_name,
                        table_name,
                        size_bytes,
                        replica_count,
                        item.clone(),
                    );
                }
            }
            _ => {}
        }
    }

    fn build_table_data_size_report(
        raw_data: &serde_json::Value,
        db_name: Option<&str>,
        table_name: Option<&str>,
        single_replica: bool,
    ) -> TableDataSizeReport {
        let mut report = TableDataSizeReport {
            summary: TableDataSizeSummary {
                total_databases: 0,
                total_tables: 0,
                total_size_bytes: 0,
                total_size_formatted: "0 B".to_string(),
                single_replica,
                query_filters: TableDataSizeFilters {
                    db_name: db_name.map(|value| value.to_string()),
                    table_name: table_name.map(|value| value.to_string()),
                },
            },
            databases: BTreeMap::new(),
        };

        match raw_data {
            serde_json::Value::Array(records) => {
                for record in records {
                    Self::process_table_data_size_record(
                        &mut report,
                        record,
                        db_name,
                        table_name,
                    );
                }
            }
            serde_json::Value::Object(root) => {
                if let Some(tables) = root.get("tables") {
                    let effective_db = db_name.unwrap_or("unknown");
                    Self::process_table_data_size_tables(
                        &mut report,
                        effective_db,
                        tables,
                        table_name,
                    );
                } else {
                    for (database_name, db_info) in root {
                        if let Some(tables) = db_info.get("tables") {
                            Self::process_table_data_size_tables(
                                &mut report,
                                database_name,
                                tables,
                                table_name,
                            );
                        } else {
                            Self::process_table_data_size_record(
                                &mut report,
                                db_info,
                                Some(database_name.as_str()),
                                table_name,
                            );
                        }
                    }
                }
            }
            _ => {}
        }

        report.summary.total_databases = report.databases.len();
        report.summary.total_tables = report
            .databases
            .values_mut()
            .map(|database| {
                database.table_count = database.tables.len();
                database.total_size_formatted = Self::format_bytes(database.total_size_bytes);
                database.table_count
            })
            .sum();
        report.summary.total_size_formatted = Self::format_bytes(report.summary.total_size_bytes);

        report
    }

    fn sanitize_query_sql(sql: &str) -> Result<String, SearcherError> {
        let trimmed = sql.trim();
        if trimmed.is_empty() {
            return Err(SearcherError::ApiError("SQL statement cannot be empty".to_string()));
        }

        let trimmed = trimmed.trim_end_matches(';').trim();
        if trimmed.contains(';') {
            return Err(SearcherError::ApiError(
                "Multiple SQL statements are not allowed".to_string(),
            ));
        }

        let tokens: Vec<String> = trimmed
            .split_whitespace()
            .map(|part| {
                part.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .to_ascii_uppercase()
            })
            .filter(|part| !part.is_empty())
            .collect();

        let first_token = tokens
            .first()
            .ok_or_else(|| SearcherError::ApiError("SQL statement cannot be empty".to_string()))?;

        match first_token.as_str() {
            "SELECT" | "SHOW" | "WITH" | "DESC" | "DESCRIBE" => {}
            "EXPLAIN" => {
                let nested_token = tokens
                    .iter()
                    .skip(1)
                    .find(|token| token.as_str() != "VERBOSE" && token.as_str() != "ANALYZE")
                    .map(|token| token.as_str());

                if matches!(
                    nested_token,
                    Some(
                        "DELETE"
                            | "INSERT"
                            | "UPDATE"
                            | "DROP"
                            | "TRUNCATE"
                            | "ALTER"
                            | "CREATE"
                            | "LOAD"
                            | "EXPORT"
                            | "INSTALL"
                            | "UNINSTALL"
                    )
                ) {
                    return Err(SearcherError::ApiError(
                        "EXPLAIN only supports read-only queries".to_string(),
                    ));
                }
            }
            _ => {
                return Err(SearcherError::ApiError(format!(
                    "Unsupported SQL statement '{}'. Only read-only SELECT/SHOW/WITH/EXPLAIN/DESC queries are allowed",
                    first_token
                )));
            }
        }

        Ok(trimmed.to_string())
    }

    async fn execute_session_statement(
        conn: &mut PoolConnection<MySql>,
        statement: &str,
    ) -> Result<(), SearcherError> {
        sqlx::query(statement)
            .execute(&mut **conn)
            .await
            .map_err(|e| SearcherError::ApiError(format!("Failed to execute '{}': {}", statement, e)))?;
        Ok(())
    }

    async fn apply_catalog_context(
        &self,
        conn: &mut PoolConnection<MySql>,
        catalog_name: Option<&str>,
    ) -> Result<String, SearcherError> {
        let effective_catalog = self.effective_catalog_name(catalog_name);
        Self::validate_identifier(effective_catalog, "catalog name")?;

        let statement = format!("USE CATALOG {}", Self::quote_identifier(effective_catalog));
        Self::execute_session_statement(conn, &statement).await?;

        Ok(effective_catalog.to_string())
    }

    async fn apply_database_context(
        &self,
        conn: &mut PoolConnection<MySql>,
        db_name: Option<&str>,
    ) -> Result<String, SearcherError> {
        let effective_db = self.effective_database_name(db_name);
        Self::validate_identifier(effective_db, "database name")?;

        let statement = format!("USE {}", Self::quote_identifier(effective_db));
        Self::execute_session_statement(conn, &statement).await?;

        Ok(effective_db.to_string())
    }

    async fn fetch_rows(
        conn: &mut PoolConnection<MySql>,
        sql: &str,
        timeout_secs: u64,
    ) -> Result<Vec<MySqlRow>, SearcherError> {
        tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            sqlx::query(sql).fetch_all(&mut **conn),
        )
        .await
        .map_err(|_| SearcherError::ApiError(format!("Query timed out after {} seconds", timeout_secs)))?
        .map_err(|e| SearcherError::ApiError(format!("Query execution failed: {}", e)))
    }

    fn build_query_result(
        rows: Vec<MySqlRow>,
        sql: &str,
        execution_time_ms: u64,
        max_rows: Option<usize>,
    ) -> QueryResult {
        if rows.is_empty() {
            return QueryResult {
                data: vec![],
                columns: vec![],
                row_count: 0,
                total_row_count: 0,
                truncated: false,
                execution_time_ms,
                sql: sql.to_string(),
            };
        }

        let columns: Vec<String> = rows[0]
            .columns()
            .iter()
            .map(|col| col.name().to_string())
            .collect();

        let mut data: Vec<HashMap<String, serde_json::Value>> = rows
            .iter()
            .map(|row| {
                let mut map = HashMap::new();
                for (i, col) in row.columns().iter().enumerate() {
                    map.insert(col.name().to_string(), Self::column_to_json(row, i));
                }
                map
            })
            .collect();

        let total_row_count = data.len();
        let truncated = max_rows.is_some_and(|limit| total_row_count > limit);
        if let Some(limit) = max_rows {
            data.truncate(limit);
        }

        QueryResult {
            row_count: data.len(),
            total_row_count,
            truncated,
            execution_time_ms,
            sql: sql.to_string(),
            columns,
            data,
        }
    }

    fn information_schema_prefix(&self, catalog_name: Option<&str>) -> Result<String, SearcherError> {
        let effective_catalog = self.effective_catalog_name(catalog_name);
        Self::validate_identifier(effective_catalog, "catalog name")?;

        Ok(format!(
            "{}.information_schema",
            Self::quote_identifier(effective_catalog)
        ))
    }

    /// Execute a SQL query and return results
    pub async fn execute_query(&self, sql: &str) -> Result<QueryResult, SearcherError> {
        self.execute_query_with_options(sql, None, None, None, None)
            .await
    }

    /// Execute a SQL query with optional catalog/database context and output controls.
    pub async fn execute_query_with_options(
        &self,
        sql: &str,
        db_name: Option<&str>,
        catalog_name: Option<&str>,
        max_rows: Option<usize>,
        timeout_secs: Option<u64>,
    ) -> Result<QueryResult, SearcherError> {
        let sanitized_sql = Self::sanitize_query_sql(sql)?;
        let timeout_secs = Self::normalize_timeout_secs(timeout_secs);
        let max_rows = Self::normalize_max_rows(max_rows);

        tracing::info!(
            "Executing Doris query: sql='{}', db={:?}, catalog={:?}, max_rows={}, timeout={}s",
            sanitized_sql,
            db_name,
            catalog_name,
            max_rows,
            timeout_secs
        );

        let start = Instant::now();
        let mut conn = self.acquire_connection().await?;
        self.apply_catalog_context(&mut conn, catalog_name).await?;
        self.apply_database_context(&mut conn, db_name).await?;

        let rows = Self::fetch_rows(&mut conn, &sanitized_sql, timeout_secs).await?;
        let execution_time_ms = start.elapsed().as_millis() as u64;

        let result = Self::build_query_result(rows, &sanitized_sql, execution_time_ms, Some(max_rows));
        tracing::info!(
            "Query returned {} rows ({} total) in {}ms",
            result.row_count,
            result.total_row_count,
            execution_time_ms
        );

        Ok(result)
    }

    /// Convert a column value to JSON value
    fn column_to_json(row: &MySqlRow, index: usize) -> serde_json::Value {
        use sqlx::Row;

        // Try different value types based on MySQL type
        // Try integers first
        if let Ok(val) = row.try_get::<i64, _>(index) {
            return serde_json::json!(val);
        }
        if let Ok(val) = row.try_get::<u64, _>(index) {
            return serde_json::json!(val);
        }
        // Try floating point
        if let Ok(val) = row.try_get::<f64, _>(index) {
            return serde_json::json!(val);
        }
        if let Ok(val) = row.try_get::<f32, _>(index) {
            return serde_json::json!(val);
        }
        // Try boolean
        if let Ok(val) = row.try_get::<bool, _>(index) {
            return serde_json::json!(val);
        }
        // Try string (handle NULL values)
        if let Ok(val) = row.try_get::<Option<String>, _>(index) {
            return match val {
                Some(v) => serde_json::json!(v),
                None => serde_json::Value::Null,
            };
        }
        // Fallback to string
        if let Ok(val) = row.try_get::<String, _>(index) {
            return serde_json::json!(val);
        }

        serde_json::Value::Null
    }

    /// Get list of databases
    pub async fn get_databases(&self) -> Result<DatabaseListResponse, SearcherError> {
        self.get_databases_with_options(None).await
    }

    /// Get list of databases under an optional catalog.
    pub async fn get_databases_with_options(
        &self,
        catalog_name: Option<&str>,
    ) -> Result<DatabaseListResponse, SearcherError> {
        let mut conn = self.acquire_connection().await?;
        let effective_catalog = self.apply_catalog_context(&mut conn, catalog_name).await?;
        let rows = Self::fetch_rows(&mut conn, "SHOW DATABASES", DEFAULT_QUERY_TIMEOUT_SECS).await?;
        let result = Self::build_query_result(rows, "SHOW DATABASES", 0, None);
        let databases: Vec<String> = result
            .data
            .iter()
            .filter_map(|row| {
                row.get("Database")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();

        let count = databases.len();
        Ok(DatabaseListResponse {
            catalog_name: Some(effective_catalog),
            databases,
            count,
        })
    }

    /// Get list of tables in a database
    pub async fn get_tables(&self, database: &str) -> Result<TableListResponse, SearcherError> {
        self.get_tables_with_options(Some(database), None).await
    }

    /// Get list of tables in a database with optional catalog context.
    pub async fn get_tables_with_options(
        &self,
        db_name: Option<&str>,
        catalog_name: Option<&str>,
    ) -> Result<TableListResponse, SearcherError> {
        let effective_db = self.effective_database_name(db_name).to_string();
        Self::validate_identifier(&effective_db, "database name")?;

        let mut conn = self.acquire_connection().await?;
        let effective_catalog = self.apply_catalog_context(&mut conn, catalog_name).await?;
        let sql = format!("SHOW TABLES FROM {}", Self::quote_identifier(&effective_db));
        let rows = Self::fetch_rows(&mut conn, &sql, DEFAULT_QUERY_TIMEOUT_SECS).await?;
        let result = Self::build_query_result(rows, &sql, 0, None);
        let tables: Vec<String> = result
            .data
            .iter()
            .filter_map(|row| {
                row.values()
                    .next()
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect();

        let count = tables.len();
        Ok(TableListResponse {
            catalog_name: Some(effective_catalog),
            database: effective_db,
            tables,
            count,
        })
    }

    /// Get table schema
    pub async fn get_table_schema(
        &self,
        database: &str,
        table: &str,
    ) -> Result<TableSchemaResponse, SearcherError> {
        self.get_table_schema_with_options(Some(database), table, None)
            .await
    }

    /// Get table schema with optional catalog/database context.
    pub async fn get_table_schema_with_options(
        &self,
        db_name: Option<&str>,
        table: &str,
        catalog_name: Option<&str>,
    ) -> Result<TableSchemaResponse, SearcherError> {
        let effective_db = self.effective_database_name(db_name).to_string();
        Self::validate_identifier(&effective_db, "database name")?;
        Self::validate_identifier(table, "table name")?;

        let information_schema = self.information_schema_prefix(catalog_name)?;
        let sql = format!(
            "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE, COLUMN_DEFAULT, COLUMN_COMMENT
             FROM {}.COLUMNS
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}'
             ORDER BY ORDINAL_POSITION",
            information_schema, effective_db, table
        );

        let result = self.execute_query(&sql).await?;
        let columns: Vec<ColumnSchema> = result
            .data
            .iter()
            .filter_map(|row| {
                Some(ColumnSchema {
                    name: row.get("COLUMN_NAME")?.as_str()?.to_string(),
                    data_type: row.get("DATA_TYPE")?.as_str()?.to_string(),
                    is_nullable: row.get("IS_NULLABLE")?.as_str()?.to_string(),
                    default_value: row
                        .get("COLUMN_DEFAULT")
                        .and_then(|v| v.as_str().map(|s| s.to_string())),
                    comment: row
                        .get("COLUMN_COMMENT")
                        .and_then(|v| v.as_str().map(|s| s.to_string())),
                })
            })
            .collect();

        let column_count = columns.len();
        Ok(TableSchemaResponse {
            catalog_name: catalog_name
                .map(|value| value.to_string())
                .or_else(|| Some(self.default_catalog.clone())),
            database: effective_db,
            table: table.to_string(),
            columns,
            column_count,
        })
    }

    /// Get table metadata including size and row count
    pub async fn get_table_metadata(
        &self,
        database: &str,
        table: &str,
    ) -> Result<TableMetadata, SearcherError> {
        self.get_table_metadata_with_options(Some(database), table, None)
            .await
    }

    /// Get table metadata with optional catalog/database context.
    pub async fn get_table_metadata_with_options(
        &self,
        db_name: Option<&str>,
        table: &str,
        catalog_name: Option<&str>,
    ) -> Result<TableMetadata, SearcherError> {
        let effective_db = self.effective_database_name(db_name).to_string();
        Self::validate_identifier(&effective_db, "database name")?;
        Self::validate_identifier(table, "table name")?;

        let information_schema = self.information_schema_prefix(catalog_name)?;
        // Query table size and row count from information_schema
        let sql = format!(
            "SELECT TABLE_ROWS, DATA_LENGTH, INDEX_LENGTH, CREATE_TIME, UPDATE_TIME
             FROM {}.TABLES
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}'",
            information_schema, effective_db, table
        );

        let result = self.execute_query(&sql).await?;

        if let Some(row) = result.data.first() {
            Ok(TableMetadata {
                catalog_name: catalog_name
                    .map(|value| value.to_string())
                    .or_else(|| Some(self.default_catalog.clone())),
                database: effective_db,
                table: table.to_string(),
                row_count: row.get("TABLE_ROWS").and_then(|v| v.as_u64()),
                data_length: row.get("DATA_LENGTH").and_then(|v| v.as_u64()),
                index_length: row.get("INDEX_LENGTH").and_then(|v| v.as_u64()),
                create_time: row
                    .get("CREATE_TIME")
                    .and_then(|v| v.as_str().map(|s| s.to_string())),
                update_time: row
                    .get("UPDATE_TIME")
                    .and_then(|v| v.as_str().map(|s| s.to_string())),
            })
        } else {
            Err(SearcherError::ApiError(format!(
                "Table not found: {}.{}",
                effective_db, table
            )))
        }
    }

    async fn get_table_size_info(
        &self,
        db_name: Option<&str>,
        table: &str,
        catalog_name: Option<&str>,
    ) -> Result<TableSizeInfo, SearcherError> {
        let effective_db = self.effective_database_name(db_name).to_string();
        Self::validate_identifier(&effective_db, "database name")?;
        Self::validate_identifier(table, "table name")?;

        let information_schema = self.information_schema_prefix(catalog_name)?;
        let sql = format!(
            "SELECT ENGINE, TABLE_ROWS, DATA_LENGTH, INDEX_LENGTH, (DATA_LENGTH + INDEX_LENGTH) AS TOTAL_SIZE
             FROM {}.TABLES
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}'",
            information_schema, effective_db, table
        );
        let result = self.execute_query(&sql).await?;
        let row = result.data.first();

        let estimated_rows = row
            .and_then(|value| value.get("TABLE_ROWS"))
            .and_then(Self::json_value_to_u64);
        let data_length = row
            .and_then(|value| value.get("DATA_LENGTH"))
            .and_then(Self::json_value_to_u64);
        let index_length = row
            .and_then(|value| value.get("INDEX_LENGTH"))
            .and_then(Self::json_value_to_u64);
        let total_size = row
            .and_then(|value| value.get("TOTAL_SIZE"))
            .and_then(Self::json_value_to_u64)
            .or_else(|| {
                Some(data_length.unwrap_or(0).saturating_add(index_length.unwrap_or(0)))
            });

        Ok(TableSizeInfo {
            engine: row
                .and_then(|value| value.get("ENGINE"))
                .and_then(Self::json_value_to_string),
            estimated_rows,
            data_length,
            index_length,
            total_size,
            total_size_formatted: total_size.map(Self::format_bytes),
        })
    }

    async fn get_table_partitions_info(
        &self,
        db_name: Option<&str>,
        table: &str,
        catalog_name: Option<&str>,
    ) -> Result<Vec<TablePartitionInfo>, SearcherError> {
        let effective_db = self.effective_database_name(db_name).to_string();
        Self::validate_identifier(&effective_db, "database name")?;
        Self::validate_identifier(table, "table name")?;

        let information_schema = self.information_schema_prefix(catalog_name)?;
        let sql = format!(
            "SELECT PARTITION_NAME, PARTITION_DESCRIPTION, TABLE_ROWS, DATA_LENGTH, INDEX_LENGTH
             FROM {}.PARTITIONS
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' AND PARTITION_NAME IS NOT NULL",
            information_schema, effective_db, table
        );
        let result = self.execute_query(&sql).await?;

        Ok(result
            .data
            .iter()
            .filter_map(|row| {
                Some(TablePartitionInfo {
                    partition_name: row
                        .get("PARTITION_NAME")
                        .and_then(Self::json_value_to_string)?,
                    partition_description: row
                        .get("PARTITION_DESCRIPTION")
                        .and_then(Self::json_value_to_string),
                    table_rows: row.get("TABLE_ROWS").and_then(Self::json_value_to_u64),
                    data_length: row.get("DATA_LENGTH").and_then(Self::json_value_to_u64),
                    index_length: row.get("INDEX_LENGTH").and_then(Self::json_value_to_u64),
                })
            })
            .collect())
    }

    /// Get basic table information for quality and governance workflows.
    pub async fn get_table_basic_info(
        &self,
        db_name: Option<&str>,
        table: &str,
        catalog_name: Option<&str>,
    ) -> Result<TableBasicInfoResponse, SearcherError> {
        let start = Instant::now();
        let effective_db = self.effective_database_name(db_name).to_string();
        let effective_catalog = self.effective_catalog_name(catalog_name).to_string();
        Self::validate_identifier(&effective_db, "database name")?;
        Self::validate_identifier(&effective_catalog, "catalog name")?;
        Self::validate_identifier(table, "table name")?;

        let schema = self
            .get_table_schema_with_options(Some(&effective_db), table, Some(&effective_catalog))
            .await?;
        let table_size = self
            .get_table_size_info(Some(&effective_db), table, Some(&effective_catalog))
            .await?;
        let partitions = self
            .get_table_partitions_info(Some(&effective_db), table, Some(&effective_catalog))
            .await
            .unwrap_or_default();

        let columns_info = schema
            .columns
            .into_iter()
            .map(|column| TableBasicColumnInfo {
                column_name: column.name,
                data_type: column.data_type,
                nullable: column.is_nullable.eq_ignore_ascii_case("yes")
                    || column.is_nullable.eq_ignore_ascii_case("true"),
                default_value: column.default_value,
                column_comment: column.comment,
            })
            .collect::<Vec<_>>();
        let row_count = table_size.estimated_rows.unwrap_or(0);
        let partition_count = partitions.len();

        Ok(TableBasicInfoResponse {
            table_name: format!("{}.{}.{}", effective_catalog, effective_db, table),
            catalog_name: effective_catalog,
            database: effective_db,
            analysis_timestamp: chrono::Utc::now().to_rfc3339(),
            row_count,
            column_count: columns_info.len(),
            columns_info,
            partitions_info: TablePartitionsInfo {
                partition_count,
                partitions,
            },
            table_size,
            execution_time_seconds: start.elapsed().as_secs_f64(),
        })
    }

    async fn get_table_ddl(
        &self,
        db_name: Option<&str>,
        table: &str,
        catalog_name: Option<&str>,
    ) -> Result<Option<String>, SearcherError> {
        let effective_db = self.effective_database_name(db_name);
        Self::validate_identifier(effective_db, "database name")?;
        Self::validate_identifier(table, "table name")?;

        let sql = format!(
            "SHOW CREATE TABLE {}",
            Self::table_reference(effective_db, table)
        );
        let mut conn = self.acquire_connection().await?;
        self.apply_catalog_context(&mut conn, catalog_name).await?;
        let rows = Self::fetch_rows(&mut conn, &sql, DEFAULT_QUERY_TIMEOUT_SECS).await?;
        let result = Self::build_query_result(rows, &sql, 0, None);
        Ok(result.data.first().and_then(|row| {
            row.values()
                .filter_map(Self::json_value_to_string)
                .max_by_key(|value| value.len())
        }))
    }

    async fn fetch_audit_log_entries(
        &self,
        days: u32,
        limit: usize,
        include_system_users: bool,
        require_metrics: bool,
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, SearcherError> {
        let since = (chrono::Local::now() - chrono::Duration::days(days as i64))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let limit = limit.max(1);
        let system_filter = if include_system_users {
            String::new()
        } else {
            " AND `user` NOT IN ('root','admin','system','doris','information_schema')".to_string()
        };

        let detailed_sql = format!(
            "SELECT client_ip, user, db, time, stmt_id, stmt, state, error_code, query_time, scan_bytes, scan_rows, return_rows
             FROM `__internal_schema`.`audit_log`
             WHERE `time` >= '{since}' AND `stmt` IS NOT NULL AND `stmt` != ''{system_filter}
             ORDER BY `time` DESC
             LIMIT {limit}"
        );
        let simple_sql = format!(
            "SELECT client_ip, user, db, time, stmt_id, stmt, state, error_code
             FROM `__internal_schema`.`audit_log`
             WHERE `time` >= '{since}' AND `stmt` IS NOT NULL AND `stmt` != ''{system_filter}
             ORDER BY `time` DESC
             LIMIT {limit}"
        );

        let primary_sql = if require_metrics {
            &detailed_sql
        } else {
            &simple_sql
        };
        let fallback_sql = if require_metrics {
            Some(&simple_sql)
        } else {
            None
        };

        match self
            .execute_query_with_options(
                primary_sql,
                None,
                Some(DEFAULT_DORIS_CATALOG),
                Some(limit),
                Some(DEFAULT_QUERY_TIMEOUT_SECS),
            )
            .await
        {
            Ok(result) => Ok(result.data),
            Err(error) => {
                if let Some(fallback_sql) = fallback_sql {
                    let fallback = self
                        .execute_query_with_options(
                            fallback_sql,
                            None,
                            Some(DEFAULT_DORIS_CATALOG),
                            Some(limit),
                            Some(DEFAULT_QUERY_TIMEOUT_SECS),
                        )
                        .await?;
                    Ok(fallback.data)
                } else {
                    Err(error)
                }
            }
        }
    }

    /// Analyze completeness and distribution for selected columns.
    pub async fn analyze_columns(
        &self,
        db_name: Option<&str>,
        table: &str,
        columns: &[String],
        analysis_types: Option<&[String]>,
        sample_size: Option<usize>,
        catalog_name: Option<&str>,
        detailed_response: bool,
    ) -> Result<serde_json::Value, SearcherError> {
        if columns.is_empty() {
            return Err(SearcherError::ApiError(
                "At least one column must be provided".to_string(),
            ));
        }

        let start = Instant::now();
        let effective_db = self.effective_database_name(db_name).to_string();
        let effective_catalog = self.effective_catalog_name(catalog_name).to_string();
        Self::validate_identifier(&effective_db, "database name")?;
        Self::validate_identifier(&effective_catalog, "catalog name")?;
        Self::validate_identifier(table, "table name")?;

        let schema = self
            .get_table_schema_with_options(Some(&effective_db), table, Some(&effective_catalog))
            .await?;
        let schema_map = schema
            .columns
            .into_iter()
            .map(|column| (column.name.clone(), column))
            .collect::<HashMap<_, _>>();

        let target_columns = columns
            .iter()
            .map(|column| {
                Self::validate_identifier(column, "column name")?;
                let schema = schema_map.get(column).ok_or_else(|| {
                    SearcherError::ApiError(format!(
                        "Column '{}' not found in {}.{}",
                        column, effective_db, table
                    ))
                })?;
                Ok((column.clone(), schema.clone()))
            })
            .collect::<Result<Vec<_>, SearcherError>>()?;

        let normalized_analysis_types = analysis_types
            .map(|types| {
                let mut normalized = types
                    .iter()
                    .map(|value| value.trim().to_ascii_lowercase())
                    .collect::<Vec<_>>();
                normalized.sort();
                normalized.dedup();
                normalized
            })
            .filter(|types| !types.is_empty())
            .unwrap_or_else(|| vec!["both".to_string()]);
        let do_completeness = normalized_analysis_types
            .iter()
            .any(|value| value == "both" || value == "completeness");
        let do_distribution = normalized_analysis_types
            .iter()
            .any(|value| value == "both" || value == "distribution");

        let sample_size = Self::normalize_analysis_sample_size(sample_size);
        let table_basic_info = self
            .get_table_basic_info(Some(&effective_db), table, Some(&effective_catalog))
            .await?;
        let total_rows = table_basic_info.row_count;
        let use_full_table = total_rows == 0 || total_rows <= sample_size as u64;
        let table_expr = if use_full_table {
            Self::table_reference(&effective_db, table)
        } else {
            format!(
                "(SELECT * FROM {} LIMIT {}) AS sample_table",
                Self::table_reference(&effective_db, table),
                sample_size
            )
        };
        let sampled_rows = if use_full_table {
            total_rows
        } else {
            sample_size as u64
        };

        let mut completeness_analysis = serde_json::Map::new();
        let mut distribution_analysis = serde_json::Map::new();

        for (column_name, column_schema) in target_columns {
            let quoted_column = Self::quote_identifier(&column_name);

            if do_completeness {
                let sql = format!(
                    "SELECT COUNT(*) AS total_rows, COUNT({column}) AS non_null_count, COUNT(DISTINCT {column}) AS distinct_count
                     FROM {table_expr}",
                    column = quoted_column,
                    table_expr = table_expr
                );
                let result = self
                    .execute_query_with_options(
                        &sql,
                        Some(&effective_db),
                        Some(&effective_catalog),
                        Some(1),
                        Some(DEFAULT_QUERY_TIMEOUT_SECS),
                    )
                    .await?;
                let row = result.data.first().cloned().unwrap_or_default();
                let total_rows_value = row
                    .get("total_rows")
                    .and_then(Self::json_value_to_u64)
                    .unwrap_or(sampled_rows);
                let non_null_count = row
                    .get("non_null_count")
                    .and_then(Self::json_value_to_u64)
                    .unwrap_or(0);
                let distinct_count = row
                    .get("distinct_count")
                    .and_then(Self::json_value_to_u64)
                    .unwrap_or(0);
                let null_count = total_rows_value.saturating_sub(non_null_count);
                let null_rate = if total_rows_value > 0 {
                    null_count as f64 / total_rows_value as f64
                } else {
                    0.0
                };
                let uniqueness_ratio = if non_null_count > 0 {
                    distinct_count as f64 / non_null_count as f64
                } else {
                    0.0
                };

                completeness_analysis.insert(
                    column_name.clone(),
                    serde_json::json!({
                        "total_rows": total_rows_value,
                        "non_null_count": non_null_count,
                        "null_count": null_count,
                        "null_rate": (null_rate * 10_000.0).round() / 10_000.0,
                        "completeness_score": ((1.0 - null_rate) * 10_000.0).round() / 10_000.0,
                        "distinct_count": distinct_count,
                        "uniqueness_ratio": (uniqueness_ratio * 10_000.0).round() / 10_000.0
                    }),
                );
            }

            if do_distribution {
                if Self::is_numeric_data_type(&column_schema.data_type) {
                    let sql = format!(
                        "SELECT MIN({column}) AS min_value, MAX({column}) AS max_value, AVG({column}) AS avg_value, STDDEV({column}) AS stddev_value
                         FROM {table_expr}",
                        column = quoted_column,
                        table_expr = table_expr
                    );
                    let result = self
                        .execute_query_with_options(
                            &sql,
                            Some(&effective_db),
                            Some(&effective_catalog),
                            Some(1),
                            Some(DEFAULT_QUERY_TIMEOUT_SECS),
                        )
                        .await?;
                    let row = result.data.first().cloned().unwrap_or_default();
                    distribution_analysis.insert(
                        column_name.clone(),
                        serde_json::json!({
                            "data_type": "numeric",
                            "min_value": row.get("min_value").cloned().unwrap_or(serde_json::Value::Null),
                            "max_value": row.get("max_value").cloned().unwrap_or(serde_json::Value::Null),
                            "mean": row.get("avg_value").cloned().unwrap_or(serde_json::Value::Null),
                            "std_dev": row.get("stddev_value").cloned().unwrap_or(serde_json::Value::Null)
                        }),
                    );
                } else if Self::is_temporal_data_type(&column_schema.data_type) {
                    let sql = format!(
                        "SELECT MIN({column}) AS min_value, MAX({column}) AS max_value
                         FROM {table_expr}",
                        column = quoted_column,
                        table_expr = table_expr
                    );
                    let result = self
                        .execute_query_with_options(
                            &sql,
                            Some(&effective_db),
                            Some(&effective_catalog),
                            Some(1),
                            Some(DEFAULT_QUERY_TIMEOUT_SECS),
                        )
                        .await?;
                    let row = result.data.first().cloned().unwrap_or_default();
                    distribution_analysis.insert(
                        column_name.clone(),
                        serde_json::json!({
                            "data_type": "temporal",
                            "min_value": row.get("min_value").cloned().unwrap_or(serde_json::Value::Null),
                            "max_value": row.get("max_value").cloned().unwrap_or(serde_json::Value::Null)
                        }),
                    );
                } else {
                    let sql = format!(
                        "SELECT {column} AS value, COUNT(*) AS frequency
                         FROM {table_expr}
                         WHERE {column} IS NOT NULL
                         GROUP BY {column}
                         ORDER BY frequency DESC
                         LIMIT 10",
                        column = quoted_column,
                        table_expr = table_expr
                    );
                    let result = self
                        .execute_query_with_options(
                            &sql,
                            Some(&effective_db),
                            Some(&effective_catalog),
                            Some(10),
                            Some(DEFAULT_QUERY_TIMEOUT_SECS),
                        )
                        .await?;
                    distribution_analysis.insert(
                        column_name.clone(),
                        serde_json::json!({
                            "data_type": if Self::is_categorical_data_type(&column_schema.data_type) {
                                "categorical"
                            } else {
                                "other"
                            },
                            "top_values": result.data,
                            "returned_values": result.row_count,
                            "detailed_response": detailed_response
                        }),
                    );
                }
            }
        }

        Ok(serde_json::json!({
            "table_name": Self::qualified_table_name(&effective_catalog, &effective_db, table),
            "analysis_timestamp": chrono::Utc::now().to_rfc3339(),
            "columns_analyzed": columns.len(),
            "analysis_types": normalized_analysis_types,
            "sampling_info": {
                "sample_size": sampled_rows,
                "sample_rate": if total_rows > 0 {
                    ((sampled_rows as f64 / total_rows as f64) * 10_000.0).round() / 10_000.0
                } else {
                    1.0
                },
                "sampling_method": if use_full_table { "full_table" } else { "limit_sampling" },
                "total_rows": total_rows
            },
            "completeness_analysis": do_completeness.then_some(serde_json::Value::Object(completeness_analysis)),
            "distribution_analysis": do_distribution.then_some(serde_json::Value::Object(distribution_analysis)),
            "execution_time_seconds": start.elapsed().as_secs_f64(),
            "detailed_response": detailed_response
        }))
    }

    /// Analyze table storage and physical distribution details.
    pub async fn analyze_table_storage(
        &self,
        db_name: Option<&str>,
        table: &str,
        catalog_name: Option<&str>,
        detailed_response: bool,
    ) -> Result<serde_json::Value, SearcherError> {
        let start = Instant::now();
        let effective_db = self.effective_database_name(db_name).to_string();
        let effective_catalog = self.effective_catalog_name(catalog_name).to_string();

        let basic_info = self
            .get_table_basic_info(Some(&effective_db), table, Some(&effective_catalog))
            .await?;
        let ddl = self
            .get_table_ddl(Some(&effective_db), table, Some(&effective_catalog))
            .await?;
        let distribution = ddl
            .as_deref()
            .map(Self::parse_distribution_from_ddl)
            .unwrap_or_else(|| serde_json::json!({}));
        let partitioning = ddl
            .as_deref()
            .map(Self::parse_partitioning_from_ddl)
            .unwrap_or_else(|| serde_json::json!({}));

        Ok(serde_json::json!({
            "table_name": basic_info.table_name,
            "analysis_timestamp": chrono::Utc::now().to_rfc3339(),
            "physical_distribution": {
                "distribution": distribution,
                "partitioning": partitioning,
                "partition_count": basic_info.partitions_info.partition_count,
                "partitions": if detailed_response {
                    Self::to_json_value(&basic_info.partitions_info.partitions)
                } else {
                    serde_json::Value::Null
                }
            },
            "storage_info": {
                "table_size": Self::to_json_value(&basic_info.table_size),
                "column_count": basic_info.column_count,
                "row_count": basic_info.row_count
            },
            "execution_time_seconds": start.elapsed().as_secs_f64(),
            "detailed_response": detailed_response,
            "ddl": detailed_response.then_some(ddl).flatten()
        }))
    }

    /// Monitor table freshness based on metadata and recent write activity.
    pub async fn monitor_data_freshness(
        &self,
        db_name: Option<&str>,
        table_names: Option<&[String]>,
        freshness_threshold_hours: u64,
        include_update_patterns: bool,
        catalog_name: Option<&str>,
    ) -> Result<serde_json::Value, SearcherError> {
        let start = Instant::now();
        let effective_db = self.effective_database_name(db_name).to_string();
        let effective_catalog = self.effective_catalog_name(catalog_name).to_string();
        let threshold_hours = freshness_threshold_hours.max(1);

        let tables = if let Some(table_names) = table_names {
            table_names
                .iter()
                .map(|table| {
                    Self::validate_identifier(table, "table name")?;
                    Ok(table.clone())
                })
                .collect::<Result<Vec<_>, SearcherError>>()?
        } else {
            self.get_tables_with_options(Some(&effective_db), Some(&effective_catalog))
                .await?
                .tables
        };

        let mut table_freshness = serde_json::Map::new();
        let mut fresh_tables = 0usize;
        let mut stale_tables = 0usize;
        let now = chrono::Utc::now().naive_utc();

        for table_name in tables {
            let metadata = self
                .get_table_metadata_with_options(
                    Some(&effective_db),
                    &table_name,
                    Some(&effective_catalog),
                )
                .await?;

            let activity_sql = format!(
                "SELECT MAX(`time`) AS last_write_time, COUNT(*) AS write_events
                 FROM `__internal_schema`.`audit_log`
                 WHERE `time` >= '{}'
                   AND LOWER(`stmt`) LIKE '%{}%'
                   AND (
                        LOWER(`stmt`) LIKE 'insert %'
                        OR LOWER(`stmt`) LIKE 'update %'
                        OR LOWER(`stmt`) LIKE 'delete %'
                        OR LOWER(`stmt`) LIKE 'load %'
                        OR LOWER(`stmt`) LIKE 'stream load%'
                        OR LOWER(`stmt`) LIKE 'merge %'
                   )",
                (chrono::Local::now() - chrono::Duration::days(30))
                    .format("%Y-%m-%d %H:%M:%S"),
                Self::escape_sql_string(&table_name.to_ascii_lowercase())
            );
            let activity = self
                .execute_query_with_options(
                    &activity_sql,
                    None,
                    Some(DEFAULT_DORIS_CATALOG),
                    Some(1),
                    Some(DEFAULT_QUERY_TIMEOUT_SECS),
                )
                .await
                .ok()
                .and_then(|result| result.data.first().cloned());

            let last_write_time = activity
                .as_ref()
                .and_then(|row| row.get("last_write_time"))
                .and_then(Self::json_value_to_string);
            let write_events = activity
                .as_ref()
                .and_then(|row| row.get("write_events"))
                .and_then(Self::json_value_to_u64)
                .unwrap_or(0);

            let freshness_candidate = last_write_time
                .clone()
                .or(metadata.update_time.clone())
                .or(metadata.create_time.clone());
            let freshness_dt = freshness_candidate
                .as_deref()
                .and_then(Self::parse_datetime_string);
            let freshness_age_hours = freshness_dt
                .map(|dt| (now - dt).num_hours().max(0) as u64);
            let is_fresh = freshness_age_hours
                .map(|hours| hours <= threshold_hours)
                .unwrap_or(false);

            if is_fresh {
                fresh_tables += 1;
            } else {
                stale_tables += 1;
            }

            let update_patterns = if include_update_patterns {
                Some(serde_json::json!({
                    "recent_write_events": write_events,
                    "last_write_time": last_write_time
                }))
            } else {
                None
            };

            table_freshness.insert(
                table_name.clone(),
                serde_json::json!({
                    "status": if is_fresh { "fresh" } else { "stale" },
                    "freshness_age_hours": freshness_age_hours,
                    "freshness_reference_time": freshness_candidate,
                    "row_count": metadata.row_count,
                    "data_length": metadata.data_length,
                    "update_patterns": update_patterns
                }),
            );
        }

        let total_tables = fresh_tables + stale_tables;
        let alerts = table_freshness
            .iter()
            .filter(|(_, value)| value.get("status").and_then(|item| item.as_str()) == Some("stale"))
            .map(|(table_name, value)| {
                serde_json::json!({
                    "table_name": table_name,
                    "severity": "medium",
                    "message": format!(
                        "Table {} is stale for {:?} hours",
                        table_name,
                        value.get("freshness_age_hours").and_then(|item| item.as_u64())
                    )
                })
            })
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "monitoring_timestamp": chrono::Utc::now().to_rfc3339(),
            "execution_time_seconds": start.elapsed().as_secs_f64(),
            "monitoring_scope": {
                "catalog_name": effective_catalog,
                "db_name": effective_db,
                "time_threshold_hours": threshold_hours
            },
            "freshness_summary": {
                "total_tables": total_tables,
                "fresh_tables": fresh_tables,
                "stale_tables": stale_tables,
                "overall_freshness_score": if total_tables > 0 {
                    ((fresh_tables as f64 / total_tables as f64) * 1000.0).round() / 1000.0
                } else {
                    0.0
                }
            },
            "table_freshness": serde_json::Value::Object(table_freshness),
            "data_flow_issues": alerts.iter().filter(|alert| alert["severity"] == "medium").cloned().collect::<Vec<_>>(),
            "alerts": alerts
        }))
    }

    /// Analyze user access patterns from Doris audit logs.
    pub async fn analyze_data_access_patterns(
        &self,
        days: Option<u32>,
        include_system_users: bool,
        min_query_threshold: Option<u64>,
    ) -> Result<serde_json::Value, SearcherError> {
        #[derive(Default)]
        struct UserAccessAccumulator {
            total_queries: u64,
            tables: BTreeSet<String>,
            hosts: BTreeSet<String>,
            query_types: BTreeMap<String, u64>,
            failed_queries: u64,
            total_exec_ms: u64,
            max_exec_ms: u64,
            total_scan_bytes: u64,
            total_scan_rows: u64,
            hourly_pattern: [u64; 24],
            example_statements: Vec<String>,
        }

        let start = Instant::now();
        let days = Self::normalize_audit_log_days(days);
        let threshold = min_query_threshold.unwrap_or(5).max(1);
        let entries = self
            .fetch_audit_log_entries(
                days,
                DEFAULT_ANALYSIS_AUDIT_LOG_LIMIT,
                include_system_users,
                true,
            )
            .await?;

        let mut users: BTreeMap<String, UserAccessAccumulator> = BTreeMap::new();
        let mut total_queries = 0u64;

        for entry in &entries {
            let user = entry
                .get("user")
                .and_then(Self::json_value_to_string)
                .unwrap_or_else(|| "unknown".to_string());
            let sql = entry
                .get("stmt")
                .and_then(Self::json_value_to_string)
                .unwrap_or_default();
            let host = entry
                .get("client_ip")
                .and_then(Self::json_value_to_string)
                .unwrap_or_default();
            let audit_db = entry
                .get("db")
                .and_then(Self::json_value_to_string)
                .unwrap_or_else(|| self.database.clone());
            let query_type = Self::classify_sql_statement(&sql);
            let execution_time_ms = entry
                .get("query_time")
                .and_then(Self::json_value_to_u64)
                .unwrap_or(0);
            let scan_bytes = entry
                .get("scan_bytes")
                .and_then(Self::json_value_to_u64)
                .unwrap_or(0);
            let scan_rows = entry
                .get("scan_rows")
                .and_then(Self::json_value_to_u64)
                .unwrap_or(0);
            let failed = entry
                .get("state")
                .and_then(Self::json_value_to_string)
                .map(|value| value.to_ascii_uppercase() != "EOF")
                .unwrap_or(false)
                || entry
                    .get("error_code")
                    .and_then(Self::json_value_to_u64)
                    .unwrap_or(0)
                    != 0;

            let accumulator = users.entry(user.clone()).or_default();
            accumulator.total_queries += 1;
            accumulator.total_exec_ms += execution_time_ms;
            accumulator.max_exec_ms = accumulator.max_exec_ms.max(execution_time_ms);
            accumulator.total_scan_bytes += scan_bytes;
            accumulator.total_scan_rows += scan_rows;
            *accumulator.query_types.entry(query_type).or_insert(0) += 1;
            if failed {
                accumulator.failed_queries += 1;
            }
            if !host.is_empty() {
                accumulator.hosts.insert(host);
            }
            for table in Self::extract_table_references_from_sql(
                &sql,
                Some(DEFAULT_DORIS_CATALOG),
                Some(&audit_db),
            ) {
                accumulator.tables.insert(table);
            }
            if let Some(query_time) = entry
                .get("time")
                .and_then(Self::json_value_to_string)
                .and_then(|value| Self::parse_datetime_string(&value))
            {
                accumulator.hourly_pattern[query_time.hour() as usize] += 1;
            }
            if accumulator.example_statements.len() < 3 && !sql.is_empty() {
                accumulator
                    .example_statements
                    .push(Self::response_preview(&sql, 240));
            }
            total_queries += 1;
        }

        let mut user_details = Vec::new();
        let mut security_alerts = Vec::new();

        for (user, stats) in users {
            if stats.total_queries < threshold {
                continue;
            }

            let avg_exec = if stats.total_queries > 0 {
                stats.total_exec_ms as f64 / stats.total_queries as f64
            } else {
                0.0
            };
            let failed_rate = if stats.total_queries > 0 {
                stats.failed_queries as f64 / stats.total_queries as f64
            } else {
                0.0
            };
            let active_hours = stats
                .hourly_pattern
                .iter()
                .enumerate()
                .filter_map(|(hour, count)| (*count > 0).then_some(hour))
                .collect::<Vec<_>>();

            let top_query_type = stats
                .query_types
                .iter()
                .max_by_key(|(_, count)| *count)
                .map(|(query_type, _)| query_type.clone());

            if stats.hosts.len() >= 4 {
                security_alerts.push(serde_json::json!({
                    "severity": "medium",
                    "type": "multi_host_access",
                    "user_name": user,
                    "message": "User accessed Doris from multiple client IPs",
                    "host_count": stats.hosts.len()
                }));
            }
            if failed_rate >= 0.3 {
                security_alerts.push(serde_json::json!({
                    "severity": "high",
                    "type": "high_failure_rate",
                    "user_name": user,
                    "message": "User has a high failed query rate",
                    "failed_query_rate": (failed_rate * 1000.0).round() / 1000.0
                }));
            }

            user_details.push(serde_json::json!({
                "user_name": user,
                "total_queries": stats.total_queries,
                "unique_tables_accessed": stats.tables.len(),
                "tables": stats.tables.into_iter().collect::<Vec<_>>(),
                "hosts": stats.hosts.into_iter().collect::<Vec<_>>(),
                "query_types": stats.query_types,
                "top_query_type": top_query_type,
                "failed_queries": stats.failed_queries,
                "failed_query_rate": (failed_rate * 1000.0).round() / 1000.0,
                "avg_execution_time_ms": (avg_exec * 100.0).round() / 100.0,
                "max_execution_time_ms": stats.max_exec_ms,
                "data_volume_read_bytes": stats.total_scan_bytes,
                "data_volume_read_rows": stats.total_scan_rows,
                "active_hours": active_hours,
                "example_statements": stats.example_statements
            }));
        }

        let total_users = user_details.len();

        Ok(serde_json::json!({
            "analysis_period": {
                "days": days,
                "start_date": (chrono::Utc::now() - chrono::Duration::days(days as i64)).to_rfc3339(),
                "end_date": chrono::Utc::now().to_rfc3339()
            },
            "analysis_timestamp": chrono::Utc::now().to_rfc3339(),
            "execution_time_seconds": start.elapsed().as_secs_f64(),
            "user_access_summary": {
                "total_users": total_users,
                "total_queries": total_queries,
                "users_meeting_threshold": total_users,
                "high_risk_alerts": security_alerts.iter().filter(|alert| alert["severity"] == "high").count()
            },
            "user_access_details": user_details,
            "role_analysis": {
                "available": false,
                "message": "Role-level enrichment is not yet available in the Rust implementation"
            },
            "security_alerts": security_alerts,
            "access_insights": {
                "include_system_users": include_system_users,
                "min_query_threshold": threshold
            },
            "recommendations": [
                "Review users with repeated failed queries or unusually broad host access.",
                "Combine this report with Doris account/role metadata if you need stronger security attribution."
            ]
        }))
    }

    /// Analyze top-N slow queries from audit logs.
    pub async fn analyze_slow_queries_topn(
        &self,
        days: Option<u32>,
        top_n: Option<usize>,
        min_execution_time_ms: Option<u64>,
        include_patterns: bool,
    ) -> Result<serde_json::Value, SearcherError> {
        let start = Instant::now();
        let days = Self::normalize_audit_log_days(days);
        let top_n = top_n.unwrap_or(20).clamp(1, 200);
        let min_execution_time_ms = min_execution_time_ms.unwrap_or(1000).max(1);
        let mut entries = self
            .fetch_audit_log_entries(days, DEFAULT_ANALYSIS_AUDIT_LOG_LIMIT, true, true)
            .await?;

        entries.retain(|entry| {
            entry
                .get("query_time")
                .and_then(Self::json_value_to_u64)
                .unwrap_or(0)
                >= min_execution_time_ms
        });
        entries.sort_by_key(|entry| {
            std::cmp::Reverse(
                entry
                    .get("query_time")
                    .and_then(Self::json_value_to_u64)
                    .unwrap_or(0),
            )
        });

        let top_queries = entries
            .iter()
            .take(top_n)
            .map(|entry| {
                let sql = entry
                    .get("stmt")
                    .and_then(Self::json_value_to_string)
                    .unwrap_or_default();
                let audit_db = entry
                    .get("db")
                    .and_then(Self::json_value_to_string);
                serde_json::json!({
                    "user_name": entry.get("user").and_then(Self::json_value_to_string),
                    "db_name": entry.get("db").and_then(Self::json_value_to_string),
                    "query_time": entry.get("time").and_then(Self::json_value_to_string),
                    "execution_time_ms": entry.get("query_time").and_then(Self::json_value_to_u64),
                    "scan_bytes": entry.get("scan_bytes").and_then(Self::json_value_to_u64),
                    "scan_rows": entry.get("scan_rows").and_then(Self::json_value_to_u64),
                    "return_rows": entry.get("return_rows").and_then(Self::json_value_to_u64),
                    "query_type": Self::classify_sql_statement(&sql),
                    "tables": Self::extract_table_references_from_sql(
                        &sql,
                        Some(DEFAULT_DORIS_CATALOG),
                        audit_db.as_deref()
                    ),
                    "sql_pattern": Self::simplify_sql_pattern(&sql),
                    "sql_preview": Self::response_preview(&sql, 400)
                })
            })
            .collect::<Vec<_>>();

        let execution_times = entries
            .iter()
            .filter_map(|entry| entry.get("query_time").and_then(Self::json_value_to_u64))
            .collect::<Vec<_>>();
        let average_execution_time_ms = if execution_times.is_empty() {
            0.0
        } else {
            execution_times.iter().sum::<u64>() as f64 / execution_times.len() as f64
        };
        let pattern_counts = if include_patterns {
            let mut counts = BTreeMap::<String, u64>::new();
            for entry in &entries {
                let sql = entry
                    .get("stmt")
                    .and_then(Self::json_value_to_string)
                    .unwrap_or_default();
                *counts.entry(Self::simplify_sql_pattern(&sql)).or_insert(0) += 1;
            }
            serde_json::json!(counts)
        } else {
            serde_json::Value::Null
        };

        Ok(serde_json::json!({
            "analysis_period": {
                "days": days,
                "threshold_ms": min_execution_time_ms,
                "start_date": (chrono::Utc::now() - chrono::Duration::days(days as i64)).to_rfc3339(),
                "end_date": chrono::Utc::now().to_rfc3339()
            },
            "analysis_timestamp": chrono::Utc::now().to_rfc3339(),
            "execution_time_seconds": start.elapsed().as_secs_f64(),
            "summary": {
                "total_slow_queries": entries.len(),
                "top_n_analyzed": top_queries.len(),
                "average_execution_time_ms": (average_execution_time_ms * 100.0).round() / 100.0,
                "max_execution_time_ms": execution_times.iter().max().copied().unwrap_or(0)
            },
            "top_slow_queries": top_queries,
            "performance_insights": {
                "patterns_included": include_patterns,
                "slow_query_types": entries.iter().fold(BTreeMap::<String, u64>::new(), |mut acc, entry| {
                    let sql = entry.get("stmt").and_then(Self::json_value_to_string).unwrap_or_default();
                    *acc.entry(Self::classify_sql_statement(&sql)).or_insert(0) += 1;
                    acc
                })
            },
            "query_patterns": pattern_counts,
            "recommendations": [
                "Focus on the highest execution time patterns first and compare scan_bytes/scan_rows against returned rows.",
                "Use doris_get_sql_explain or doris_get_sql_profile on the worst queries to inspect execution plans."
            ]
        }))
    }

    /// Analyze resource growth trends from current storage snapshot and audit logs.
    pub async fn analyze_resource_growth_curves(
        &self,
        days: Option<u32>,
        resource_types: Option<&[String]>,
        include_predictions: bool,
        detailed_response: bool,
    ) -> Result<serde_json::Value, SearcherError> {
        let start = Instant::now();
        let days = Self::normalize_audit_log_days(days);
        let resource_types = resource_types
            .map(|values| values.iter().map(|value| value.to_ascii_lowercase()).collect::<Vec<_>>())
            .filter(|values| !values.is_empty())
            .unwrap_or_else(|| vec!["storage".to_string(), "query_volume".to_string(), "user_activity".to_string()]);
        let entries = self
            .fetch_audit_log_entries(days, DEFAULT_ANALYSIS_AUDIT_LOG_LIMIT, true, true)
            .await?;

        let mut daily_query_volume = BTreeMap::<String, u64>::new();
        let mut daily_user_activity = BTreeMap::<String, BTreeSet<String>>::new();
        for entry in &entries {
            let Some(time_value) = entry.get("time").and_then(Self::json_value_to_string) else {
                continue;
            };
            let date_key = time_value.chars().take(10).collect::<String>();
            *daily_query_volume.entry(date_key.clone()).or_insert(0) += 1;
            let user = entry
                .get("user")
                .and_then(Self::json_value_to_string)
                .unwrap_or_else(|| "unknown".to_string());
            daily_user_activity.entry(date_key).or_default().insert(user);
        }

        let query_series = daily_query_volume
            .iter()
            .map(|(date, count)| serde_json::json!({"date": date, "total_queries": count}))
            .collect::<Vec<_>>();
        let user_series = daily_user_activity
            .iter()
            .map(|(date, users)| serde_json::json!({"date": date, "unique_users": users.len()}))
            .collect::<Vec<_>>();

        let storage_snapshot = if resource_types.iter().any(|value| value == "storage") {
            Some(self.get_table_data_size(None, None, false).await?)
        } else {
            None
        };

        let mut resource_analysis = serde_json::Map::new();
        if resource_types.iter().any(|value| value == "storage") {
            resource_analysis.insert(
                "storage".to_string(),
                serde_json::json!({
                    "growth_trend": "snapshot_only",
                    "current_snapshot": storage_snapshot,
                    "note": "Historical storage growth is approximated by current FE snapshot only in the Rust implementation"
                }),
            );
        }
        if resource_types.iter().any(|value| value == "query_volume") {
            let current = daily_query_volume.values().last().copied().unwrap_or(0);
            let average = if daily_query_volume.is_empty() {
                0.0
            } else {
                daily_query_volume.values().sum::<u64>() as f64 / daily_query_volume.len() as f64
            };
            resource_analysis.insert(
                "query_volume".to_string(),
                serde_json::json!({
                    "growth_trend": if current as f64 >= average { "upward_or_stable" } else { "downward" },
                    "daily_query_count": {
                        "current": current,
                        "average": (average * 100.0).round() / 100.0
                    },
                    "daily_data": detailed_response.then_some(query_series.clone())
                }),
            );
        }
        if resource_types.iter().any(|value| value == "user_activity") {
            let current = daily_user_activity.values().last().map(|users| users.len()).unwrap_or(0);
            let average = if daily_user_activity.is_empty() {
                0.0
            } else {
                daily_user_activity.values().map(|users| users.len()).sum::<usize>() as f64
                    / daily_user_activity.len() as f64
            };
            resource_analysis.insert(
                "user_activity".to_string(),
                serde_json::json!({
                    "growth_trend": if current as f64 >= average { "upward_or_stable" } else { "downward" },
                    "daily_active_users": {
                        "current": current,
                        "average": (average * 100.0).round() / 100.0
                    },
                    "daily_data": detailed_response.then_some(user_series.clone())
                }),
            );
        }

        let growth_predictions = if include_predictions {
            let query_prediction = if daily_query_volume.len() >= 2 {
                let first = daily_query_volume.values().next().copied().unwrap_or(0) as f64;
                let last = daily_query_volume.values().last().copied().unwrap_or(0) as f64;
                Some(((last - first) / daily_query_volume.len() as f64 * 7.0 * 100.0).round() / 100.0)
            } else {
                None
            };
            serde_json::json!({
                "query_volume_next_7d_delta_estimate": query_prediction,
                "note": "Predictions use a simple linear delta estimate from observed daily counts"
            })
        } else {
            serde_json::Value::Null
        };

        Ok(serde_json::json!({
            "analysis_period": {
                "days": days,
                "start_date": (chrono::Utc::now() - chrono::Duration::days(days as i64)).to_rfc3339(),
                "end_date": chrono::Utc::now().to_rfc3339()
            },
            "analysis_timestamp": chrono::Utc::now().to_rfc3339(),
            "execution_time_seconds": start.elapsed().as_secs_f64(),
            "resource_types_analyzed": resource_types,
            "resource_analysis": serde_json::Value::Object(resource_analysis),
            "growth_insights": {
                "query_days_observed": daily_query_volume.len(),
                "user_activity_days_observed": daily_user_activity.len()
            },
            "growth_predictions": growth_predictions,
            "recommendations": [
                "Use this report for directional trends; storage growth remains a current snapshot until historical storage series is available.",
                "Correlate query volume spikes with slow-query and access-pattern reports for operational follow-up."
            ],
            "_execution_info": {
                "tool_name": "analyze_resource_growth_curves",
                "detailed_response": detailed_response
            }
        }))
    }

    /// Analyze table dependency graph from audit-log-observed SQL relationships.
    pub async fn analyze_data_flow_dependencies(
        &self,
        db_name: Option<&str>,
        target_table: Option<&str>,
        analysis_depth: Option<u32>,
        include_views: bool,
        catalog_name: Option<&str>,
    ) -> Result<serde_json::Value, SearcherError> {
        #[derive(Default)]
        struct DependencyNode {
            upstream: BTreeSet<String>,
            downstream: BTreeSet<String>,
            evidence: Vec<String>,
        }

        let start = Instant::now();
        let effective_catalog = self.effective_catalog_name(catalog_name).to_string();
        let effective_db = self.effective_database_name(db_name).to_string();
        let depth = analysis_depth.unwrap_or(3).max(1) as usize;
        let entries = self
            .fetch_audit_log_entries(30, DEFAULT_ANALYSIS_AUDIT_LOG_LIMIT, true, false)
            .await?;

        let mut graph: BTreeMap<String, DependencyNode> = BTreeMap::new();

        for entry in &entries {
            let sql = entry
                .get("stmt")
                .and_then(Self::json_value_to_string)
                .unwrap_or_default();
            let audit_db = entry
                .get("db")
                .and_then(Self::json_value_to_string)
                .unwrap_or_else(|| effective_db.clone());
            let refs = Self::extract_table_references_from_sql(
                &sql,
                Some(&effective_catalog),
                Some(&audit_db),
            );
            let dest = Self::extract_destination_table_from_sql(
                &sql,
                Some(&effective_catalog),
                Some(&audit_db),
            );

            if let Some(destination) = dest {
                let destination_node = graph.entry(destination.clone()).or_default();
                if destination_node.evidence.len() < 3 {
                    destination_node
                        .evidence
                        .push(Self::response_preview(&sql, 220));
                }

                for source in refs.into_iter().filter(|source| source != &destination) {
                    graph.entry(destination.clone()).or_default().upstream.insert(source.clone());
                    graph.entry(source.clone()).or_default().downstream.insert(destination.clone());
                }
            } else if include_views && refs.len() > 1 {
                for source in &refs {
                    for target in &refs {
                        if source != target {
                            graph.entry(source.clone()).or_default().downstream.insert(target.clone());
                            graph.entry(target.clone()).or_default().upstream.insert(source.clone());
                        }
                    }
                }
            }
        }

        let graph_stats = serde_json::json!({
            "total_tables": graph.len(),
            "total_edges": graph.values().map(|node| node.downstream.len()).sum::<usize>(),
        });

        let target_table = target_table.and_then(|table| {
            Self::canonicalize_table_reference(table, Some(&effective_catalog), Some(&effective_db))
        });

        if let Some(target_table_name) = target_table {
            let traverse = |direction: &str| {
                let mut visited = BTreeSet::new();
                let mut queue = VecDeque::from([(target_table_name.clone(), 0usize)]);
                let mut result = Vec::new();

                while let Some((current, current_depth)) = queue.pop_front() {
                    if current_depth >= depth {
                        continue;
                    }
                    let neighbors = graph
                        .get(&current)
                        .map(|node| {
                            if direction == "upstream" {
                                node.upstream.iter().cloned().collect::<Vec<_>>()
                            } else {
                                node.downstream.iter().cloned().collect::<Vec<_>>()
                            }
                        })
                        .unwrap_or_default();

                    for neighbor in neighbors {
                        if visited.insert(neighbor.clone()) {
                            result.push(serde_json::json!({
                                "table_name": neighbor,
                                "depth": current_depth + 1
                            }));
                            queue.push_back((neighbor, current_depth + 1));
                        }
                    }
                }

                result
            };

            let upstream = traverse("upstream");
            let downstream = traverse("downstream");
            let evidence = graph
                .get(&target_table_name)
                .map(|node| node.evidence.clone())
                .unwrap_or_default();
            let upstream_count = graph
                .get(&target_table_name)
                .map(|node| node.upstream.len())
                .unwrap_or(0);
            let downstream_count = graph
                .get(&target_table_name)
                .map(|node| node.downstream.len())
                .unwrap_or(0);
            let confidence = if graph.contains_key(&target_table_name) {
                "medium"
            } else {
                "low"
            };

            return Ok(serde_json::json!({
                "analysis_target": target_table_name,
                "analysis_timestamp": chrono::Utc::now().to_rfc3339(),
                "execution_time_seconds": start.elapsed().as_secs_f64(),
                "tables_analyzed": graph.len(),
                "dependency_graph_stats": graph_stats,
                "table_dependencies": {
                    "upstream_dependencies": upstream,
                    "downstream_dependencies": downstream,
                    "evidence": evidence
                },
                "impact_analysis": {
                    "upstream_count": upstream_count,
                    "downstream_count": downstream_count,
                    "analysis_depth": depth
                },
                "dependency_insights": {
                    "include_views": include_views,
                    "confidence": confidence
                },
                "recommendations": [
                    "Treat runtime SQL co-occurrence as heuristic dependency evidence rather than strict lineage.",
                    "Validate critical downstream tables manually before using this graph for change impact assessment."
                ]
            }));
        }

        let all_tables = graph
            .iter()
            .map(|(table_name, node)| {
                serde_json::json!({
                    "table_name": table_name,
                    "upstream_dependencies": node.upstream.iter().cloned().collect::<Vec<_>>(),
                    "downstream_dependencies": node.downstream.iter().cloned().collect::<Vec<_>>(),
                    "dependency_count": node.upstream.len() + node.downstream.len()
                })
            })
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "analysis_target": "all_tables",
            "analysis_timestamp": chrono::Utc::now().to_rfc3339(),
            "execution_time_seconds": start.elapsed().as_secs_f64(),
            "tables_analyzed": graph.len(),
            "dependency_graph_stats": graph_stats,
            "table_dependencies": all_tables,
            "impact_analysis": {
                "high_fanout_tables": graph.iter()
                    .filter(|(_, node)| node.downstream.len() >= 3)
                    .map(|(table_name, node)| serde_json::json!({
                        "table_name": table_name,
                        "downstream_count": node.downstream.len()
                    }))
                    .collect::<Vec<_>>()
            },
            "dependency_insights": {
                "include_views": include_views
            },
            "recommendations": [
                "Investigate high fan-out tables first; they tend to be the riskiest change points.",
                "Use target_table mode for a tighter impact report on a single dataset."
            ]
        }))
    }

    /// Trace column lineage heuristically from audit-log-observed SQL statements.
    pub async fn trace_column_lineage(
        &self,
        target_columns: &[String],
        analysis_depth: Option<u32>,
        include_transformations: bool,
        catalog_name: Option<&str>,
    ) -> Result<serde_json::Value, SearcherError> {
        let effective_catalog = self.effective_catalog_name(catalog_name).to_string();
        let default_db = self.database.clone();
        let depth = analysis_depth.unwrap_or(3).max(1);
        let entries = self
            .fetch_audit_log_entries(30, DEFAULT_ANALYSIS_AUDIT_LOG_LIMIT, true, false)
            .await?;

        let mut results = serde_json::Map::new();

        for column_spec in target_columns {
            let parts = column_spec.split('.').collect::<Vec<_>>();
            let (db_name, table_name, column_name) = match parts.as_slice() {
                [table_name, column_name] => (default_db.as_str(), *table_name, *column_name),
                [db_name, table_name, column_name] => (*db_name, *table_name, *column_name),
                _ => {
                    results.insert(
                        column_spec.clone(),
                        serde_json::json!({
                            "error": "Invalid column specification. Expected table.column or db.table.column"
                        }),
                    );
                    continue;
                }
            };

            let schema = self
                .get_table_schema_with_options(Some(db_name), table_name, Some(&effective_catalog))
                .await?;
            if !schema.columns.iter().any(|column| column.name == column_name) {
                results.insert(
                    column_spec.clone(),
                    serde_json::json!({
                        "error": format!("Column {} not found in {}.{}", column_name, db_name, table_name)
                    }),
                );
                continue;
            }

            let target_table =
                Self::qualified_table_name(&effective_catalog, db_name, table_name);
            let mut source_chain = Vec::new();
            let mut downstream_usage = Vec::new();
            let mut transformations = BTreeSet::new();

            for entry in &entries {
                let sql = entry
                    .get("stmt")
                    .and_then(Self::json_value_to_string)
                    .unwrap_or_default();
                let sql_lower = sql.to_ascii_lowercase();
                if !sql_lower.contains(&table_name.to_ascii_lowercase())
                    || !sql_lower.contains(&column_name.to_ascii_lowercase())
                {
                    continue;
                }

                let audit_db = entry
                    .get("db")
                    .and_then(Self::json_value_to_string)
                    .unwrap_or_else(|| db_name.to_string());
                let refs = Self::extract_table_references_from_sql(
                    &sql,
                    Some(&effective_catalog),
                    Some(&audit_db),
                );
                let destination = Self::extract_destination_table_from_sql(
                    &sql,
                    Some(&effective_catalog),
                    Some(&audit_db),
                );

                if destination.as_deref() == Some(target_table.as_str()) {
                    for source in refs.iter().filter(|source| *source != &target_table) {
                        source_chain.push(serde_json::json!({
                            "source_table": source,
                            "source_column": column_name,
                            "relationship_type": "upstream",
                            "confidence": "medium",
                            "evidence_sql": Self::response_preview(&sql, 240)
                        }));
                    }
                    if include_transformations {
                        for transformation in Self::extract_select_transformations(&sql, column_name) {
                            transformations.insert(transformation);
                        }
                    }
                } else if refs.iter().any(|reference| reference == &target_table) {
                    downstream_usage.push(serde_json::json!({
                        "query_time": entry.get("time").and_then(Self::json_value_to_string),
                        "query_type": Self::classify_sql_statement(&sql),
                        "referenced_tables": refs,
                        "sql_preview": Self::response_preview(&sql, 220)
                    }));
                }
            }

            source_chain.sort_by_key(|item| item["source_table"].to_string());
            source_chain.dedup_by(|left, right| left["source_table"] == right["source_table"]);
            downstream_usage.truncate((depth as usize) * 10);
            let upstream_count = source_chain.len();
            let downstream_count = downstream_usage.len();
            let lineage_confidence = if upstream_count > 0 { "medium" } else { "low" };
            let risk_level = if downstream_count >= 5 { "high" } else { "medium" };
            let transformation_rules = transformations.into_iter().collect::<Vec<_>>();

            results.insert(
                column_spec.clone(),
                serde_json::json!({
                    "target_column": format!("{}.{}", target_table, column_name),
                    "analysis_timestamp": chrono::Utc::now().to_rfc3339(),
                    "lineage_depth": depth,
                    "source_chain": source_chain,
                    "downstream_usage": downstream_usage,
                    "transformation_rules": transformation_rules,
                    "lineage_confidence": lineage_confidence,
                    "impact_analysis": {
                        "upstream_dependencies": upstream_count,
                        "downstream_dependencies": downstream_count,
                        "risk_level": risk_level
                    }
                }),
            );
        }

        Ok(serde_json::json!({
            "multi_column_lineage": true,
            "column_count": target_columns.len(),
            "analysis_timestamp": chrono::Utc::now().to_rfc3339(),
            "results": serde_json::Value::Object(results)
        }))
    }

    /// Get ADBC / Arrow Flight SQL connection diagnostics.
    pub async fn get_adbc_connection_info(&self) -> Result<serde_json::Value, SearcherError> {
        let fe_port_raw = std::env::var("FE_ARROW_FLIGHT_SQL_PORT").ok();
        let be_port_raw = std::env::var("BE_ARROW_FLIGHT_SQL_PORT").ok();
        let fe_port = fe_port_raw
            .as_deref()
            .and_then(|value| value.parse::<u16>().ok());
        let be_port = be_port_raw
            .as_deref()
            .and_then(|value| value.parse::<u16>().ok());

        let (backend_hosts, backend_discovery_error) =
            match self.discover_backend_monitoring_nodes().await {
                Ok(nodes) => {
                    let hosts = nodes
                        .into_iter()
                        .map(|node| node.host)
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect::<Vec<_>>();
                    (hosts, None)
                }
                Err(error) => (Vec::new(), Some(error.to_string())),
            };

        let fe_connectivity = if let Some(port) = fe_port {
            Some(serde_json::json!({
                "host": self.host,
                "port": port,
                "available": Self::check_tcp_endpoint(&self.host, port, 3).await
            }))
        } else {
            None
        };

        let mut backend_port_checks = Vec::new();
        if let Some(port) = be_port {
            for host in backend_hosts.iter().take(3) {
                backend_port_checks.push(serde_json::json!({
                    "host": host,
                    "port": port,
                    "available": Self::check_tcp_endpoint(host, port, 3).await
                }));
            }
        }

        let fe_available = fe_connectivity
            .as_ref()
            .and_then(|value| value.get("available"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let be_available = backend_port_checks
            .iter()
            .any(|value| value.get("available").and_then(|item| item.as_bool()) == Some(true));
        let ports_configured = fe_port.is_some() && be_port.is_some();

        let (status, message) = if !ports_configured {
            (
                "not_configured",
                "Missing FE_ARROW_FLIGHT_SQL_PORT or BE_ARROW_FLIGHT_SQL_PORT configuration",
            )
        } else if !fe_available {
            (
                "not_ready",
                "FE Arrow Flight SQL endpoint is not reachable from the current Rust process",
            )
        } else if !backend_port_checks.is_empty() && !be_available {
            (
                "not_ready",
                "No discovered BE Arrow Flight SQL endpoint is reachable from the current Rust process",
            )
        } else {
            (
                "diagnostic_ready",
                "Ports look reachable, but this Rust build does not link a native Arrow Flight SQL / ADBC driver yet",
            )
        };

        Ok(serde_json::json!({
            "status": status,
            "message": message,
            "adbc_available": false,
            "execution_supported": false,
            "configuration": {
                "fe_host": self.host,
                "fe_arrow_flight_sql_port": fe_port_raw,
                "be_arrow_flight_sql_port": be_port_raw,
                "user": self.username,
                "default_database": self.database,
                "default_catalog": self.default_catalog
            },
            "connectivity": {
                "ports_configured": ports_configured,
                "fe_endpoint": fe_connectivity,
                "discovered_be_hosts": backend_hosts,
                "be_endpoint_checks": backend_port_checks,
                "backend_discovery_error": backend_discovery_error
            },
            "implementation": {
                "mode": "diagnostic_only",
                "rust_native_adbc_driver_linked": false,
                "compatible_query_fallback": "doris_exec_query",
                "notes": [
                    "This repository exposes ADBC-compatible diagnostics, but not native Arrow Flight SQL execution in Rust.",
                    "Use doris_exec_query for production reads until a Rust Flight SQL driver is integrated."
                ]
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Execute a compatibility query for the official exec_adbc_query surface.
    pub async fn exec_adbc_query(
        &self,
        sql: &str,
        max_rows: Option<usize>,
        timeout: Option<u64>,
        return_format: Option<&str>,
    ) -> Result<serde_json::Value, SearcherError> {
        let requested_max_rows = max_rows.unwrap_or(100_000);
        let applied_max_rows = Self::normalize_max_rows(Some(requested_max_rows));
        let requested_timeout = timeout.unwrap_or(60);
        let applied_timeout = Self::normalize_timeout_secs(Some(requested_timeout));
        let requested_return_format = return_format
            .unwrap_or("dict")
            .trim()
            .to_ascii_lowercase();

        if !matches!(requested_return_format.as_str(), "arrow" | "pandas" | "dict") {
            return Err(SearcherError::ApiError(format!(
                "Unsupported return_format '{}', expected one of: arrow, pandas, dict",
                requested_return_format
            )));
        }

        let connection_info = self.get_adbc_connection_info().await?;
        let result = self
            .execute_query_with_options(
                sql,
                None,
                None,
                Some(applied_max_rows),
                Some(applied_timeout),
            )
            .await?;

        Ok(serde_json::json!({
            "success": true,
            "protocol": "mysql_compatibility_fallback",
            "requested_protocol": "ADBC_Arrow_Flight_SQL",
            "adbc_available": false,
            "requested_return_format": requested_return_format,
            "returned_format": "dict",
            "format_downgraded": requested_return_format != "dict",
            "max_rows_requested": requested_max_rows,
            "max_rows_applied": applied_max_rows,
            "timeout_requested_seconds": requested_timeout,
            "timeout_applied_seconds": applied_timeout,
            "fallback_reason": "Arrow Flight SQL / ADBC execution is not linked in this Rust build, so the query ran through the existing Doris MySQL connection.",
            "result": {
                "format": "dict",
                "num_rows": result.row_count,
                "num_columns": result.columns.len(),
                "column_names": result.columns,
                "data": result.data,
                "execution_time_ms": result.execution_time_ms,
                "truncated": result.truncated,
                "sql": result.sql
            },
            "connection_info": connection_info,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }

    /// Get table comment information.
    pub async fn get_table_comment(
        &self,
        db_name: Option<&str>,
        table: &str,
        catalog_name: Option<&str>,
    ) -> Result<TableCommentResponse, SearcherError> {
        let effective_db = self.effective_database_name(db_name).to_string();
        Self::validate_identifier(&effective_db, "database name")?;
        Self::validate_identifier(table, "table name")?;

        let information_schema = self.information_schema_prefix(catalog_name)?;
        let sql = format!(
            "SELECT TABLE_COMMENT FROM {}.TABLES
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}'",
            information_schema, effective_db, table
        );

        let result = self.execute_query(&sql).await?;
        let comment = result
            .data
            .first()
            .and_then(|row| row.get("TABLE_COMMENT"))
            .and_then(|value| value.as_str())
            .map(|value| value.to_string());

        Ok(TableCommentResponse {
            catalog_name: catalog_name
                .map(|value| value.to_string())
                .or_else(|| Some(self.default_catalog.clone())),
            database: effective_db,
            table: table.to_string(),
            comment,
        })
    }

    /// Get table indexes.
    pub async fn get_table_indexes(
        &self,
        db_name: Option<&str>,
        table: &str,
        catalog_name: Option<&str>,
    ) -> Result<QueryResult, SearcherError> {
        let effective_db = self.effective_database_name(db_name);
        Self::validate_identifier(effective_db, "database name")?;
        Self::validate_identifier(table, "table name")?;

        let sql = format!(
            "SHOW INDEX FROM {}.{}",
            Self::quote_identifier(effective_db),
            Self::quote_identifier(table)
        );

        let mut conn = self.acquire_connection().await?;
        self.apply_catalog_context(&mut conn, catalog_name).await?;
        let rows = Self::fetch_rows(&mut conn, &sql, DEFAULT_QUERY_TIMEOUT_SECS).await?;
        Ok(Self::build_query_result(rows, &sql, 0, None))
    }

    /// Get SQL execution plan.
    pub async fn get_sql_explain(
        &self,
        sql: &str,
        verbose: bool,
        db_name: Option<&str>,
        catalog_name: Option<&str>,
    ) -> Result<QueryResult, SearcherError> {
        let explain_sql = if verbose {
            format!("EXPLAIN VERBOSE {}", sql.trim())
        } else {
            format!("EXPLAIN {}", sql.trim())
        };

        self.execute_query_with_options(&explain_sql, db_name, catalog_name, None, None)
            .await
    }

    /// Get a list of all catalogs.
    pub async fn get_catalog_list(&self) -> Result<CatalogListResponse, SearcherError> {
        let mut conn = self.acquire_connection().await?;
        let rows = Self::fetch_rows(&mut conn, "SHOW CATALOGS", DEFAULT_QUERY_TIMEOUT_SECS).await?;
        let result = Self::build_query_result(rows, "SHOW CATALOGS", 0, None);
        let catalogs = result
            .data
            .iter()
            .map(|row| CatalogInfo {
                catalog_id: row.get("CatalogId").and_then(|value| value.as_str()).map(|value| value.to_string()),
                catalog_name: row
                    .get("CatalogName")
                    .or_else(|| row.get("Catalog"))
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
                type_: row.get("Type").and_then(|value| value.as_str()).map(|value| value.to_string()),
                is_current: row.get("IsCurrent").and_then(|value| value.as_str()).map(|value| value.to_string()),
                create_time: row.get("CreateTime").and_then(|value| value.as_str()).map(|value| value.to_string()),
                last_update_time: row
                    .get("LastUpdateTime")
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string()),
                comment: row.get("Comment").and_then(|value| value.as_str()).map(|value| value.to_string()),
            })
            .collect::<Vec<_>>();

        Ok(CatalogListResponse {
            count: catalogs.len(),
            catalogs,
        })
    }

    /// Get recent audit logs from Doris internal audit table.
    pub async fn get_recent_audit_logs(
        &self,
        days: Option<u32>,
        limit: Option<usize>,
    ) -> Result<AuditLogResponse, SearcherError> {
        let days = Self::normalize_audit_log_days(days);
        let limit = Self::normalize_audit_log_limit(limit);
        let since_date = (chrono::Local::now() - chrono::Duration::days(days as i64))
            .format("%Y-%m-%d")
            .to_string();

        let sql = format!(
            "SELECT client_ip, user, db, time, stmt_id, stmt, state, error_code \
             FROM `__internal_schema`.`audit_log` \
             WHERE `time` >= '{since_date}' \
             AND state = 'EOF' AND error_code = 0 \
             AND `stmt` NOT LIKE 'SHOW%' \
             AND `stmt` NOT LIKE 'DESC%' \
             AND `stmt` NOT LIKE 'DESCRIBE%' \
             AND `stmt` NOT LIKE 'EXPLAIN%' \
             AND `stmt` NOT LIKE 'SELECT 1%' \
             ORDER BY `time` DESC \
             LIMIT {limit}"
        );

        let result = self
            .execute_query_with_options(
                &sql,
                None,
                Some(DEFAULT_DORIS_CATALOG),
                Some(limit),
                Some(DEFAULT_QUERY_TIMEOUT_SECS),
            )
            .await?;

        Ok(AuditLogResponse {
            days,
            limit,
            since_date,
            count: result.row_count,
            logs: result.data,
        })
    }

    /// Get SQL execution profile via Doris FE HTTP API.
    pub async fn get_sql_profile(
        &self,
        sql: &str,
        db_name: Option<&str>,
        catalog_name: Option<&str>,
        timeout_secs: Option<u64>,
    ) -> Result<SqlProfileResponse, SearcherError> {
        if self.http_client.is_none() || self.http_url.is_none() {
            return Err(SearcherError::ApiError(
                "HTTP API URL not configured. Please set DORIS_HTTP_URL.".to_string(),
            ));
        }

        let sanitized_sql = Self::sanitize_query_sql(sql)?;
        let timeout_secs = Self::normalize_timeout_secs(timeout_secs);
        let trace_id = Uuid::new_v4().to_string();
        let mut conn = self.acquire_connection().await?;
        let effective_catalog = self.apply_catalog_context(&mut conn, catalog_name).await?;
        let effective_db = self.apply_database_context(&mut conn, db_name).await?;

        let session_context = format!("SET session_context=\"trace_id:{}\"", trace_id);
        Self::execute_session_statement(&mut conn, &session_context).await?;
        Self::execute_session_statement(&mut conn, "SET enable_profile=true").await?;

        let start = Instant::now();
        let rows = Self::fetch_rows(&mut conn, &sanitized_sql, timeout_secs).await?;
        let execution_time_ms = start.elapsed().as_millis() as u64;
        let query_result =
            Self::build_query_result(rows, &sanitized_sql, execution_time_ms, Some(DEFAULT_QUERY_MAX_ROWS));

        let _ = Self::execute_session_statement(&mut conn, "SET enable_profile=false").await;
        drop(conn);

        let summary = SqlResultSummary {
            row_count: query_result.total_row_count,
            returned_row_count: query_result.row_count,
            truncated: query_result.truncated,
            columns: query_result.columns.clone(),
        };

        let Some(query_id) = self.get_query_id_by_trace_id(&trace_id).await? else {
            return Ok(SqlProfileResponse {
                success: false,
                trace_id,
                query_id: None,
                sql: sanitized_sql,
                database: effective_db,
                catalog_name: effective_catalog,
                execution_time_ms,
                sql_result_summary: summary,
                profile_text: None,
                profile_endpoint: None,
                retrieved_at: None,
                error: Some("Failed to resolve query ID from trace ID".to_string()),
            });
        };

        match self.get_profile_by_query_id(&query_id).await? {
            Some((profile_text, profile_endpoint, retrieved_at)) => Ok(SqlProfileResponse {
                success: true,
                trace_id,
                query_id: Some(query_id),
                sql: sanitized_sql,
                database: effective_db,
                catalog_name: effective_catalog,
                execution_time_ms,
                sql_result_summary: summary,
                profile_text: Some(profile_text),
                profile_endpoint: Some(profile_endpoint),
                retrieved_at: Some(retrieved_at),
                error: None,
            }),
            None => Ok(SqlProfileResponse {
                success: false,
                trace_id,
                query_id: Some(query_id),
                sql: sanitized_sql,
                database: effective_db,
                catalog_name: effective_catalog,
                execution_time_ms,
                sql_result_summary: summary,
                profile_text: None,
                profile_endpoint: None,
                retrieved_at: None,
                error: Some("Profile data not available from Doris FE HTTP API".to_string()),
            }),
        }
    }

    /// Get table data size information from Doris FE HTTP API.
    pub async fn get_table_data_size(
        &self,
        db_name: Option<&str>,
        table_name: Option<&str>,
        single_replica: bool,
    ) -> Result<TableDataSizeResponse, SearcherError> {
        if let Some(db_name) = db_name {
            Self::validate_identifier(db_name, "database name")?;
        }
        if let Some(table_name) = table_name {
            Self::validate_identifier(table_name, "table name")?;
        }

        let client = self
            .http_client
            .as_ref()
            .ok_or_else(|| SearcherError::ApiError("Doris HTTP client not configured".to_string()))?;
        let http_url = self
            .http_url
            .as_ref()
            .ok_or_else(|| SearcherError::ApiError("Doris HTTP URL not configured".to_string()))?;
        let url = format!("{}/api/show_table_data", http_url.trim_end_matches('/'));
        let timestamp = chrono::Utc::now().to_rfc3339();

        let mut query_params = Vec::new();
        if let Some(db_name) = db_name {
            query_params.push(("db", db_name.to_string()));
        }
        if let Some(table_name) = table_name {
            query_params.push(("table", table_name.to_string()));
        }
        if single_replica {
            query_params.push(("single_replica", "true".to_string()));
        }

        let response = client
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .query(&query_params)
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            return Ok(TableDataSizeResponse {
                success: false,
                db_name: db_name.map(|value| value.to_string()),
                table_name: table_name.map(|value| value.to_string()),
                single_replica,
                timestamp,
                data: None,
                url: url.clone(),
                note: None,
                error: Some(format!("HTTP request failed with status {}", status)),
                raw_response_preview: Some(Self::response_preview(&body, 500)),
            });
        }

        let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
            SearcherError::ApiError(format!("Failed to parse table data size response: {}", e))
        })?;

        if json.get("code").and_then(|value| value.as_i64()) != Some(0) {
            let error = json
                .get("msg")
                .or_else(|| json.get("message"))
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
                .unwrap_or_else(|| format!("API returned error payload: {}", json));

            return Ok(TableDataSizeResponse {
                success: false,
                db_name: db_name.map(|value| value.to_string()),
                table_name: table_name.map(|value| value.to_string()),
                single_replica,
                timestamp,
                data: None,
                url: url.clone(),
                note: None,
                error: Some(error),
                raw_response_preview: Some(Self::response_preview(&body, 500)),
            });
        }

        let Some(raw_data) = json.get("data") else {
            return Ok(TableDataSizeResponse {
                success: false,
                db_name: db_name.map(|value| value.to_string()),
                table_name: table_name.map(|value| value.to_string()),
                single_replica,
                timestamp,
                data: None,
                url: url.clone(),
                note: None,
                error: Some("Doris FE HTTP API returned empty data".to_string()),
                raw_response_preview: Some(Self::response_preview(&body, 500)),
            });
        };

        Ok(TableDataSizeResponse {
            success: true,
            db_name: db_name.map(|value| value.to_string()),
            table_name: table_name.map(|value| value.to_string()),
            single_replica,
            timestamp,
            data: Some(Self::build_table_data_size_report(
                raw_data,
                db_name,
                table_name,
                single_replica,
            )),
            url,
            note: Some("Table data size information from Doris FE HTTP API".to_string()),
            error: None,
            raw_response_preview: None,
        })
    }

    fn build_realtime_memory_stats(
        tracker_type: &str,
        include_details: bool,
    ) -> RealtimeMemoryStats {
        RealtimeMemoryStats {
            success: true,
            tracker_type: tracker_type.to_string(),
            include_details,
            timestamp: chrono::Utc::now().to_rfc3339(),
            memory_stats: serde_json::json!({
                "total_memory": "8.00 GB",
                "used_memory": "4.50 GB",
                "free_memory": "3.50 GB",
                "memory_usage_percent": 56.25
            }),
            note: Some(
                "Memory tracker functionality requires Doris BE HTTP endpoints to be available"
                    .to_string(),
            ),
            error: None,
        }
    }

    fn build_historical_memory_stats(
        tracker_names: Option<Vec<String>>,
        time_range: &str,
    ) -> HistoricalMemoryStats {
        HistoricalMemoryStats {
            success: true,
            tracker_names,
            time_range: time_range.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            historical_stats: serde_json::json!({
                "data_points": 60,
                "interval": "1m",
                "memory_trend": "stable",
                "avg_usage": "4.2 GB",
                "peak_usage": "5.1 GB",
                "min_usage": "3.8 GB"
            }),
            note: Some(
                "Historical memory tracking functionality requires Doris BE bvar endpoints to be available"
                    .to_string(),
            ),
            error: None,
        }
    }

    /// Get Doris memory statistics. This currently mirrors the official placeholder tool behavior.
    pub async fn get_memory_stats(
        &self,
        data_type: Option<&str>,
        tracker_type: Option<&str>,
        tracker_names: Option<Vec<String>>,
        time_range: Option<&str>,
        include_details: bool,
    ) -> Result<MemoryStatsResponse, SearcherError> {
        let data_type = data_type.unwrap_or("realtime");
        let tracker_type = tracker_type.unwrap_or("overview");
        let time_range = time_range.unwrap_or("1h");

        match data_type {
            "realtime" => Ok(MemoryStatsResponse {
                success: true,
                data_type: data_type.to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                realtime: Some(Self::build_realtime_memory_stats(
                    tracker_type,
                    include_details,
                )),
                historical: None,
                execution_info: None,
                error: None,
            }),
            "historical" => Ok(MemoryStatsResponse {
                success: true,
                data_type: data_type.to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                realtime: None,
                historical: Some(Self::build_historical_memory_stats(
                    tracker_names,
                    time_range,
                )),
                execution_info: None,
                error: None,
            }),
            "both" => {
                let realtime =
                    Self::build_realtime_memory_stats(tracker_type, include_details);
                let historical =
                    Self::build_historical_memory_stats(tracker_names, time_range);

                Ok(MemoryStatsResponse {
                    success: true,
                    data_type: data_type.to_string(),
                    timestamp: realtime.timestamp.clone(),
                    realtime: Some(realtime),
                    historical: Some(historical),
                    execution_info: Some(MemoryStatsExecutionInfo {
                        combined_response: true,
                        realtime_available: true,
                        historical_available: true,
                    }),
                    error: None,
                })
            }
            _ => Ok(MemoryStatsResponse {
                success: false,
                data_type: data_type.to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                realtime: None,
                historical: None,
                execution_info: None,
                error: Some(format!(
                    "Invalid data_type: {}. Must be 'realtime', 'historical', or 'both'",
                    data_type
                )),
            }),
        }
    }

    fn build_metric_definitions(
        entries: &[(&str, &str, &str, &str)],
    ) -> BTreeMap<String, MonitoringMetricDefinition> {
        entries
            .iter()
            .map(|(name, meaning, description, unit)| {
                (
                    (*name).to_string(),
                    MonitoringMetricDefinition {
                        name: (*name).to_string(),
                        meaning: (*meaning).to_string(),
                        description: (*description).to_string(),
                        unit: (*unit).to_string(),
                    },
                )
            })
            .collect()
    }

    fn core_metric_definitions() -> BTreeMap<String, MonitoringMetricDefinition> {
        Self::build_metric_definitions(&[
            (
                "doris_fe_connection_total",
                "Current FE MySQL connection count",
                "Used to observe query connection pressure on FE",
                "Num",
            ),
            (
                "doris_fe_query_total",
                "Total query count on FE",
                "Can be used to estimate overall query load",
                "Num",
            ),
            (
                "doris_fe_cpu",
                "FE CPU metrics",
                "Used to observe FE CPU utilization",
                "Percentage",
            ),
            (
                "jvm_heap_size_bytes",
                "JVM heap size",
                "Used to observe FE JVM heap usage",
                "Bytes",
            ),
            (
                "doris_be_tablet_base_max_compaction_score",
                "BE base compaction score",
                "Used to observe base compaction backlog",
                "Num",
            ),
            (
                "doris_be_tablet_cumulative_max_compaction_score",
                "BE cumulative compaction score",
                "Used to observe cumulative compaction backlog",
                "Num",
            ),
            (
                "doris_be_cpu",
                "BE CPU metrics",
                "Used to observe BE CPU utilization",
                "Percentage",
            ),
            (
                "doris_be_memory_allocated_bytes",
                "BE allocated memory",
                "Used to observe BE process memory usage",
                "Bytes",
            ),
            (
                "doris_be_disk_io_util",
                "BE disk IO utilization",
                "Used to observe BE disk pressure",
                "Percentage",
            ),
            (
                "doris_be_network_receive_bytes",
                "BE network received bytes",
                "Used to observe BE inbound network traffic",
                "Bytes",
            ),
            (
                "doris_be_network_send_bytes",
                "BE network sent bytes",
                "Used to observe BE outbound network traffic",
                "Bytes",
            ),
            (
                "doris_be_query_scan_bytes",
                "BE query scanned bytes",
                "Used to observe query scan volume",
                "Bytes",
            ),
        ])
    }

    fn fe_process_metric_definitions() -> BTreeMap<String, MonitoringMetricDefinition> {
        Self::build_metric_definitions(&[
            (
                "doris_fe_connection_total",
                "Current FE MySQL connection count",
                "Used to observe query connection pressure on FE",
                "Num",
            ),
            (
                "doris_fe_query_total",
                "Total query count on FE",
                "Used to observe FE query throughput",
                "Num",
            ),
            (
                "doris_fe_query_err",
                "Total FE query errors",
                "Used to observe FE query failures",
                "Num",
            ),
            (
                "doris_fe_edit_log",
                "FE edit log metrics",
                "Used to observe FE metadata log write and read behavior",
                "Bytes/Num",
            ),
            (
                "doris_fe_max_tablet_compaction_score",
                "Max tablet compaction score",
                "Used to observe cluster compaction backlog from FE view",
                "Num",
            ),
            (
                "doris_fe_report_queue_size",
                "FE report queue size",
                "Used to observe FE task queue pressure",
                "Num",
            ),
            (
                "doris_fe_scheduled_tablet_num",
                "Scheduled tablet count",
                "Used to observe tablets waiting for scheduling",
                "Num",
            ),
            (
                "doris_fe_txn_counter",
                "FE transaction counters",
                "Used to observe transaction begin/success/reject/fail counts",
                "Num",
            ),
        ])
    }

    fn fe_jvm_metric_definitions() -> BTreeMap<String, MonitoringMetricDefinition> {
        Self::build_metric_definitions(&[
            (
                "jvm_heap_size_bytes",
                "JVM heap size",
                "Used to observe FE JVM heap usage",
                "Bytes",
            ),
            (
                "jvm_non_heap_size_bytes",
                "JVM non-heap size",
                "Used to observe FE non-heap memory usage",
                "Bytes",
            ),
            (
                "jvm_old_size_bytes",
                "JVM old generation size",
                "Used to observe old generation memory pressure",
                "Bytes",
            ),
            (
                "jvm_young_size_bytes",
                "JVM young generation size",
                "Used to observe young generation memory pressure",
                "Bytes",
            ),
            (
                "jvm_old_gc",
                "Old generation GC metrics",
                "Used to observe full GC count and duration",
                "Num/Ms",
            ),
            (
                "jvm_young_gc",
                "Young generation GC metrics",
                "Used to observe young GC count and duration",
                "Num/Ms",
            ),
        ])
    }

    fn fe_machine_metric_definitions() -> BTreeMap<String, MonitoringMetricDefinition> {
        Self::build_metric_definitions(&[
            (
                "doris_fe_cpu",
                "FE CPU metrics",
                "Used to observe FE CPU utilization",
                "Percentage",
            ),
            (
                "doris_fe_memory",
                "FE memory metrics",
                "Used to observe FE host memory usage",
                "Bytes",
            ),
            (
                "doris_fe_fd_num_limit",
                "FE file descriptor limit",
                "Used to observe FE file descriptor capacity",
                "Num",
            ),
            (
                "doris_fe_fd_num_used",
                "FE used file descriptors",
                "Used to observe FE file descriptor usage",
                "Num",
            ),
        ])
    }

    fn be_process_metric_definitions() -> BTreeMap<String, MonitoringMetricDefinition> {
        Self::build_metric_definitions(&[
            (
                "doris_be_tablet_base_max_compaction_score",
                "BE base compaction score",
                "Used to observe base compaction backlog",
                "Num",
            ),
            (
                "doris_be_tablet_cumulative_max_compaction_score",
                "BE cumulative compaction score",
                "Used to observe cumulative compaction backlog",
                "Num",
            ),
            (
                "doris_be_query_scan_bytes",
                "BE query scanned bytes",
                "Used to observe query scan volume",
                "Bytes",
            ),
            (
                "doris_be_query_scan_rows",
                "BE query scanned rows",
                "Used to observe query scan row count",
                "Num",
            ),
            (
                "doris_be_engine_requests_total",
                "BE engine requests",
                "Used to observe storage engine request volume",
                "Num",
            ),
            (
                "doris_be_stream_load",
                "BE stream load metrics",
                "Used to observe stream load throughput",
                "Bytes/Rows",
            ),
            (
                "doris_be_load_channel_count",
                "BE load channel count",
                "Used to observe active load channel pressure",
                "Num",
            ),
            (
                "doris_be_process_thread_num",
                "BE process thread count",
                "Used to observe BE thread pressure",
                "Num",
            ),
            (
                "doris_be_process_fd_num_limit_soft",
                "BE process FD soft limit",
                "Used to observe BE process file descriptor capacity",
                "Num",
            ),
            (
                "doris_be_process_fd_num_used",
                "BE process used FDs",
                "Used to observe BE process file descriptor usage",
                "Num",
            ),
        ])
    }

    fn be_machine_metric_definitions() -> BTreeMap<String, MonitoringMetricDefinition> {
        Self::build_metric_definitions(&[
            (
                "doris_be_cpu",
                "BE CPU metrics",
                "Used to observe BE CPU utilization",
                "Percentage",
            ),
            (
                "doris_be_memory_allocated_bytes",
                "BE allocated memory",
                "Used to observe BE process memory usage",
                "Bytes",
            ),
            (
                "doris_be_disks_local_used_capacity",
                "BE used disk capacity",
                "Used to observe BE local storage usage",
                "Bytes",
            ),
            (
                "doris_be_disks_total_capacity",
                "BE total disk capacity",
                "Used to observe BE local storage capacity",
                "Bytes",
            ),
            (
                "doris_be_network_receive_bytes",
                "BE network received bytes",
                "Used to observe BE inbound traffic",
                "Bytes",
            ),
            (
                "doris_be_network_send_bytes",
                "BE network sent bytes",
                "Used to observe BE outbound traffic",
                "Bytes",
            ),
            (
                "doris_be_load_average",
                "BE load average",
                "Used to observe BE host load",
                "Num",
            ),
            (
                "doris_be_fd_num_limit",
                "BE system FD limit",
                "Used to observe host file descriptor capacity",
                "Num",
            ),
            (
                "doris_be_fd_num_used",
                "BE used system FDs",
                "Used to observe host file descriptor usage",
                "Num",
            ),
            (
                "doris_be_disk_io_util",
                "BE disk IO utilization",
                "Used to observe BE disk pressure",
                "Percentage",
            ),
        ])
    }

    fn merge_metric_definition_sets(
        definition_sets: Vec<BTreeMap<String, MonitoringMetricDefinition>>,
    ) -> BTreeMap<String, MonitoringMetricDefinition> {
        let mut merged = BTreeMap::new();
        for definition_set in definition_sets {
            merged.extend(definition_set);
        }
        merged
    }

    fn monitoring_definitions_for_scope(
        role: &str,
        monitor_type: &str,
        priority: &str,
    ) -> (BTreeMap<String, MonitoringMetricDefinition>, Option<String>) {
        let definitions = match priority {
            "core" => {
                let core = Self::core_metric_definitions();
                let mut selected = BTreeMap::new();

                if role == "fe" || role == "all" {
                    if monitor_type == "process" || monitor_type == "all" {
                        for key in ["doris_fe_connection_total", "doris_fe_query_total"] {
                            if let Some(value) = core.get(key) {
                                selected.insert(key.to_string(), value.clone());
                            }
                        }
                    }
                    if monitor_type == "jvm" || monitor_type == "all" {
                        if let Some(value) = core.get("jvm_heap_size_bytes") {
                            selected.insert("jvm_heap_size_bytes".to_string(), value.clone());
                        }
                    }
                    if monitor_type == "machine" || monitor_type == "all" {
                        if let Some(value) = core.get("doris_fe_cpu") {
                            selected.insert("doris_fe_cpu".to_string(), value.clone());
                        }
                    }
                }

                if role == "be" || role == "all" {
                    if monitor_type == "process" || monitor_type == "all" {
                        for key in [
                            "doris_be_tablet_base_max_compaction_score",
                            "doris_be_tablet_cumulative_max_compaction_score",
                            "doris_be_query_scan_bytes",
                        ] {
                            if let Some(value) = core.get(key) {
                                selected.insert(key.to_string(), value.clone());
                            }
                        }
                    }
                    if monitor_type == "machine" || monitor_type == "all" {
                        for key in [
                            "doris_be_cpu",
                            "doris_be_memory_allocated_bytes",
                            "doris_be_disk_io_util",
                            "doris_be_network_receive_bytes",
                            "doris_be_network_send_bytes",
                        ] {
                            if let Some(value) = core.get(key) {
                                selected.insert(key.to_string(), value.clone());
                            }
                        }
                    }
                }

                selected
            }
            "p0" | "all" => {
                let mut definition_sets = Vec::new();
                if role == "fe" || role == "all" {
                    match monitor_type {
                        "process" => definition_sets.push(Self::fe_process_metric_definitions()),
                        "jvm" => definition_sets.push(Self::fe_jvm_metric_definitions()),
                        "machine" => definition_sets.push(Self::fe_machine_metric_definitions()),
                        _ => {
                            definition_sets.push(Self::fe_process_metric_definitions());
                            definition_sets.push(Self::fe_jvm_metric_definitions());
                            definition_sets.push(Self::fe_machine_metric_definitions());
                        }
                    }
                }
                if role == "be" || role == "all" {
                    match monitor_type {
                        "process" => definition_sets.push(Self::be_process_metric_definitions()),
                        "jvm" => {}
                        "machine" => definition_sets.push(Self::be_machine_metric_definitions()),
                        _ => {
                            definition_sets.push(Self::be_process_metric_definitions());
                            definition_sets.push(Self::be_machine_metric_definitions());
                        }
                    }
                }
                Self::merge_metric_definition_sets(definition_sets)
            }
            _ => BTreeMap::new(),
        };

        let note = (priority == "all").then_some(
            "Definitions currently cover curated core/P0 Doris metrics; data mode can still return all exposed endpoint metrics."
                .to_string(),
        );

        (definitions, note)
    }

    fn filter_metrics_by_definitions(
        metrics: &BTreeMap<String, MonitoringMetricValue>,
        definitions: &BTreeMap<String, MonitoringMetricDefinition>,
    ) -> BTreeMap<String, MonitoringMetricValue> {
        metrics
            .iter()
            .filter(|(metric_name, _)| {
                definitions.contains_key(*metric_name)
                    || definitions
                        .keys()
                        .any(|candidate| metric_name.starts_with(candidate))
            })
            .map(|(metric_name, metric_value)| (metric_name.clone(), metric_value.clone()))
            .collect()
    }

    fn metric_matches_labels(
        labels: &BTreeMap<String, String>,
        expected: &[(&str, &str)],
    ) -> bool {
        expected
            .iter()
            .all(|(key, value)| labels.get(*key).map(|item| item.as_str()) == Some(*value))
    }

    fn simple_metric_value(
        metrics: &BTreeMap<String, MonitoringMetricValue>,
        metric_name: &str,
        label_filters: &[(&str, &str)],
    ) -> Option<f64> {
        match metrics.get(metric_name) {
            Some(MonitoringMetricValue::Number(value)) => Some(*value),
            Some(MonitoringMetricValue::Samples(samples)) => {
                if label_filters.is_empty() {
                    Some(samples.iter().map(|sample| sample.value).sum())
                } else {
                    samples
                        .iter()
                        .find(|sample| Self::metric_matches_labels(&sample.labels, label_filters))
                        .map(|sample| sample.value)
                }
            }
            None => None,
        }
    }

    fn calculate_jvm_heap_usage_percent(
        metrics: &BTreeMap<String, MonitoringMetricValue>,
    ) -> Option<f64> {
        let used = Self::simple_metric_value(metrics, "jvm_heap_size_bytes", &[("type", "used")])?;
        let max = Self::simple_metric_value(metrics, "jvm_heap_size_bytes", &[("type", "max")])?;
        (max > 0.0).then_some((used / max * 100.0 * 100.0).round() / 100.0)
    }

    fn calculate_gc_average_time(
        metrics: &BTreeMap<String, MonitoringMetricValue>,
        metric_name: &str,
    ) -> Option<f64> {
        let count = Self::simple_metric_value(metrics, metric_name, &[("type", "count")])?;
        let time = Self::simple_metric_value(metrics, metric_name, &[("type", "time")])?;
        (count > 0.0).then_some((time / count * 100.0).round() / 100.0)
    }

    fn calculate_disk_usage_percent(
        metrics: &BTreeMap<String, MonitoringMetricValue>,
    ) -> Option<f64> {
        let used = Self::simple_metric_value(metrics, "doris_be_disks_local_used_capacity", &[])?;
        let total = Self::simple_metric_value(metrics, "doris_be_disks_total_capacity", &[])?;
        (total > 0.0).then_some((used / total * 100.0 * 100.0).round() / 100.0)
    }

    fn calculate_fd_usage_percent(
        metrics: &BTreeMap<String, MonitoringMetricValue>,
        used_metric: &str,
        limit_metric: &str,
    ) -> Option<f64> {
        let used = Self::simple_metric_value(metrics, used_metric, &[])?;
        let limit = Self::simple_metric_value(metrics, limit_metric, &[])?;
        (limit > 0.0).then_some((used / limit * 100.0 * 100.0).round() / 100.0)
    }

    fn calculate_cpu_usage_percent(
        metrics: &BTreeMap<String, MonitoringMetricValue>,
        metric_name: &str,
    ) -> Option<f64> {
        let MonitoringMetricValue::Samples(samples) = metrics.get(metric_name)? else {
            return None;
        };

        let mut total = 0.0;
        let mut idle = 0.0;
        for sample in samples {
            if sample.labels.get("device").map(|value| value.as_str()) != Some("cpu") {
                continue;
            }
            total += sample.value;
            if sample.labels.get("mode").map(|value| value.as_str()) == Some("idle") {
                idle += sample.value;
            }
        }

        (total > 0.0).then_some((((1.0 - idle / total) * 100.0) * 100.0).round() / 100.0)
    }

    fn aggregate_network_bytes(
        metrics: &BTreeMap<String, MonitoringMetricValue>,
        metric_name: &str,
    ) -> Option<f64> {
        match metrics.get(metric_name) {
            Some(MonitoringMetricValue::Number(value)) => Some(*value),
            Some(MonitoringMetricValue::Samples(samples)) => Some(
                samples
                    .iter()
                    .filter(|sample| sample.labels.get("device").map(|value| value.as_str()) != Some("lo"))
                    .map(|sample| sample.value)
                    .sum(),
            ),
            None => None,
        }
    }

    fn calculate_fe_dashboard_metrics(
        metrics: &BTreeMap<String, MonitoringMetricValue>,
    ) -> BTreeMap<String, f64> {
        let mut dashboard = BTreeMap::new();

        if let Some(value) = Self::simple_metric_value(metrics, "doris_fe_query_total", &[]) {
            dashboard.insert("query_total_rate".to_string(), value);
        }
        if let Some(value) =
            Self::simple_metric_value(metrics, "doris_fe_query_latency_ms", &[("quantile", "0.99")])
        {
            dashboard.insert("query_latency_99p_ms".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "doris_fe_query_err", &[]) {
            dashboard.insert("query_error_count".to_string(), value);
            dashboard.insert("query_error_rate".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "doris_fe_connection_total", &[]) {
            dashboard.insert("connection_total".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "doris_fe_request_total", &[]) {
            dashboard.insert("request_total_rate".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "jvm_heap_size_bytes", &[("type", "used")]) {
            dashboard.insert("jvm_heap_used_bytes".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "jvm_heap_size_bytes", &[("type", "max")]) {
            dashboard.insert("jvm_heap_max_bytes".to_string(), value);
        }
        if let Some(value) = Self::calculate_jvm_heap_usage_percent(metrics) {
            dashboard.insert("jvm_heap_usage_percent".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "jvm_old_gc", &[("type", "count")]) {
            dashboard.insert("jvm_old_gc_count".to_string(), value);
        }
        if let Some(value) = Self::calculate_gc_average_time(metrics, "jvm_old_gc") {
            dashboard.insert("jvm_old_gc_avg_time".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "jvm_young_gc", &[("type", "count")]) {
            dashboard.insert("jvm_young_gc_count".to_string(), value);
        }
        if let Some(value) = Self::calculate_gc_average_time(metrics, "jvm_young_gc") {
            dashboard.insert("jvm_young_gc_avg_time".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "doris_fe_tablet_max_compaction_score", &[]) {
            dashboard.insert("tablet_max_compaction_score".to_string(), value);
        }
        if let Some(value) =
            Self::simple_metric_value(metrics, "doris_fe_tablet_status_count", &[("type", "unhealthy")])
        {
            dashboard.insert("tablet_unhealthy_count".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "doris_fe_scheduled_tablet_num", &[]) {
            dashboard.insert("tablet_scheduled_num".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "doris_fe_txn_counter", &[("type", "begin")]) {
            dashboard.insert("txn_begin_total".to_string(), value);
            dashboard.insert("txn_begin_rate".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "doris_fe_txn_counter", &[("type", "success")]) {
            dashboard.insert("txn_success_total".to_string(), value);
            dashboard.insert("txn_success_rate".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "doris_fe_txn_counter", &[("type", "reject")]) {
            dashboard.insert("txn_reject_rate".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "doris_fe_txn_counter", &[("type", "failed")]) {
            dashboard.insert("txn_failed_rate".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "doris_fe_edit_log", &[("type", "write")]) {
            dashboard.insert("edit_log_write_rate".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "doris_fe_edit_log", &[("type", "read")]) {
            dashboard.insert("edit_log_read_rate".to_string(), value);
        }
        if let Some(value) =
            Self::simple_metric_value(metrics, "doris_fe_editlog_write_latency_ms", &[("quantile", "0.99")])
        {
            dashboard.insert("edit_log_write_latency_99p_ms".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "doris_fe_report_queue_size", &[]) {
            dashboard.insert("report_queue_size".to_string(), value);
        }

        dashboard
    }

    fn calculate_be_dashboard_metrics(
        metrics: &BTreeMap<String, MonitoringMetricValue>,
    ) -> BTreeMap<String, f64> {
        let mut dashboard = BTreeMap::new();

        if let Some(value) =
            Self::simple_metric_value(metrics, "doris_be_stream_load", &[("type", "receive_bytes")])
        {
            dashboard.insert("stream_load_receive_bytes_rate".to_string(), value);
        }
        if let Some(value) =
            Self::simple_metric_value(metrics, "doris_be_stream_load", &[("type", "load_rows")])
        {
            dashboard.insert("stream_load_rows_rate".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "doris_be_stream_load_txn_request", &[]) {
            dashboard.insert("stream_load_txn_request_rate".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(
            metrics,
            "doris_be_engine_requests_total",
            &[("type", "publish"), ("status", "total")],
        ) {
            dashboard.insert("engine_publish_total".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(
            metrics,
            "doris_be_engine_requests_total",
            &[("type", "publish"), ("status", "failed")],
        ) {
            dashboard.insert("engine_publish_failed_rate".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "doris_be_disks_local_used_capacity", &[]) {
            dashboard.insert("disks_used_capacity_bytes".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "doris_be_disks_total_capacity", &[]) {
            dashboard.insert("disks_total_capacity_bytes".to_string(), value);
        }
        if let Some(value) = Self::calculate_disk_usage_percent(metrics) {
            dashboard.insert("disks_usage_percent".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "doris_be_memory_allocated_bytes", &[]) {
            dashboard.insert("memory_allocated_bytes".to_string(), value);
        }
        if let Some(value) =
            Self::simple_metric_value(metrics, "doris_be_memory_jemalloc_active_bytes", &[])
        {
            dashboard.insert("memory_jemalloc_active_bytes".to_string(), value);
        }
        if let Some(value) =
            Self::simple_metric_value(metrics, "doris_be_memory_jemalloc_allocated_bytes", &[])
        {
            dashboard.insert("memory_jemalloc_allocated_bytes".to_string(), value);
        }
        if let Some(value) =
            Self::simple_metric_value(metrics, "doris_be_memory_jemalloc_resident_bytes", &[])
        {
            dashboard.insert("memory_jemalloc_resident_bytes".to_string(), value);
        }
        if let Some(value) =
            Self::simple_metric_value(metrics, "doris_be_process_fd_num_limit_soft", &[])
        {
            dashboard.insert("process_fd_num_limit_soft".to_string(), value);
        }
        if let Some(value) = Self::simple_metric_value(metrics, "doris_be_process_fd_num_used", &[]) {
            dashboard.insert("process_fd_num_used".to_string(), value);
        }
        if let Some(value) = Self::calculate_fd_usage_percent(
            metrics,
            "doris_be_process_fd_num_used",
            "doris_be_process_fd_num_limit_soft",
        ) {
            dashboard.insert("process_fd_usage_percent".to_string(), value);
        }
        if let Some(value) = Self::calculate_cpu_usage_percent(metrics, "doris_be_cpu") {
            dashboard.insert("cpu_usage_percent".to_string(), value);
        }
        if let Some(value) = Self::aggregate_network_bytes(metrics, "doris_be_network_receive_bytes") {
            dashboard.insert("network_receive_bytes_total".to_string(), value);
        }
        if let Some(value) = Self::aggregate_network_bytes(metrics, "doris_be_network_send_bytes") {
            dashboard.insert("network_send_bytes_total".to_string(), value);
        }

        dashboard
    }

    fn calculate_monitoring_summary(
        metrics: &BTreeMap<String, MonitoringMetricValue>,
        role: &str,
    ) -> serde_json::Value {
        if role == "fe" {
            let mut summary = serde_json::Map::new();
            if let Some(value) = Self::simple_metric_value(metrics, "doris_fe_query_total", &[]) {
                summary.insert("total_queries".to_string(), serde_json::json!(value));
            }
            if let Some(value) = Self::simple_metric_value(metrics, "doris_fe_query_err", &[]) {
                summary.insert("total_query_errors".to_string(), serde_json::json!(value));
            }
            if let (Some(total), Some(errors)) = (
                Self::simple_metric_value(metrics, "doris_fe_query_total", &[]),
                Self::simple_metric_value(metrics, "doris_fe_query_err", &[]),
            ) {
                if total > 0.0 {
                    summary.insert(
                        "query_error_rate_percent".to_string(),
                        serde_json::json!((((errors / total) * 100.0) * 100.0).round() / 100.0),
                    );
                }
            }
            if let Some(value) = Self::simple_metric_value(metrics, "doris_fe_connection_total", &[]) {
                summary.insert("current_connections".to_string(), serde_json::json!(value));
            }
            if let Some(value) =
                Self::simple_metric_value(metrics, "doris_fe_max_tablet_compaction_score", &[])
            {
                summary.insert("max_compaction_score".to_string(), serde_json::json!(value));
            }
            if let Some(value) = Self::simple_metric_value(metrics, "doris_fe_report_queue_size", &[]) {
                summary.insert("report_queue_size".to_string(), serde_json::json!(value));
            }
            serde_json::Value::Object(summary)
        } else {
            let mut summary = serde_json::Map::new();
            if let Some(value) = Self::calculate_cpu_usage_percent(metrics, "doris_be_cpu") {
                summary.insert("cpu_usage_percent".to_string(), serde_json::json!(value));
            }
            if let Some(value) = Self::simple_metric_value(metrics, "doris_be_memory_allocated_bytes", &[]) {
                summary.insert(
                    "memory_allocated_gb".to_string(),
                    serde_json::json!(((value / 1024f64.powi(3)) * 100.0).round() / 100.0),
                );
            }
            if let Some(value) = Self::calculate_disk_usage_percent(metrics) {
                summary.insert("disk_usage_percent".to_string(), serde_json::json!(value));
            }
            if let Some(value) = Self::calculate_fd_usage_percent(
                metrics,
                "doris_be_fd_num_used",
                "doris_be_fd_num_limit",
            ) {
                summary.insert("fd_usage_percent".to_string(), serde_json::json!(value));
            }
            if let Some(value) = Self::simple_metric_value(metrics, "doris_be_process_thread_num", &[]) {
                summary.insert("thread_count".to_string(), serde_json::json!(value));
            }
            serde_json::Value::Object(summary)
        }
    }

    async fn fetch_metrics_from_url(
        &self,
        url: &str,
    ) -> Result<BTreeMap<String, MonitoringMetricValue>, SearcherError> {
        let client = self
            .http_client
            .as_ref()
            .ok_or_else(|| SearcherError::ApiError("Doris HTTP client not configured".to_string()))?;
        let response = client
            .get(url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(SearcherError::ApiError(format!(
                "Failed to fetch Doris monitoring metrics from {}: HTTP {}",
                url,
                response.status()
            )));
        }

        let body = response.text().await?;
        Ok(Self::parse_prometheus_metrics(&body))
    }

    async fn discover_backend_nodes_via_show_backends(
        &self,
    ) -> Result<Vec<MonitoringBackendNode>, SearcherError> {
        let result = self.execute_query("SHOW BACKENDS").await?;
        let nodes = result
            .data
            .iter()
            .filter_map(|row| {
                let host = row.get("Host").and_then(Self::json_value_to_string)?;
                let http_port = row.get("HttpPort").and_then(Self::json_value_to_u64)? as u16;
                let alive = row
                    .get("Alive")
                    .and_then(Self::json_value_to_bool)
                    .unwrap_or(true);
                Some(MonitoringBackendNode {
                    backend_id: row.get("BackendId").and_then(Self::json_value_to_string),
                    host,
                    http_port,
                    alive,
                    source: "show_backends".to_string(),
                })
            })
            .collect::<Vec<_>>();

        Ok(nodes)
    }

    async fn discover_backend_nodes_via_http(
        &self,
    ) -> Result<Vec<MonitoringBackendNode>, SearcherError> {
        let http_url = self
            .http_url
            .as_ref()
            .ok_or_else(|| SearcherError::ApiError("Doris HTTP URL not configured".to_string()))?;
        let client = self
            .http_client
            .as_ref()
            .ok_or_else(|| SearcherError::ApiError("Doris HTTP client not configured".to_string()))?;
        let url = format!("{}/api/backends", http_url.trim_end_matches('/'));
        let response = client
            .get(&url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(SearcherError::ApiError(format!(
                "Failed to fetch Doris backends via HTTP: HTTP {}",
                response.status()
            )));
        }

        let json: serde_json::Value = response.json().await?;
        let backends = json
            .get("backends")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();

        Ok(backends
            .iter()
            .filter_map(|backend| {
                let host = backend
                    .get("host")
                    .or_else(|| backend.get("Host"))
                    .and_then(Self::json_value_to_string)?;
                let http_port = backend
                    .get("http_port")
                    .or_else(|| backend.get("HttpPort"))
                    .or_else(|| backend.get("webserver_port"))
                    .or_else(|| backend.get("webserverPort"))
                    .and_then(Self::json_value_to_u64)? as u16;
                let alive = backend
                    .get("alive")
                    .or_else(|| backend.get("Alive"))
                    .and_then(Self::json_value_to_bool)
                    .unwrap_or(true);

                Some(MonitoringBackendNode {
                    backend_id: backend
                        .get("backend_id")
                        .or_else(|| backend.get("BackendId"))
                        .and_then(Self::json_value_to_string),
                    host,
                    http_port,
                    alive,
                    source: "api_backends".to_string(),
                })
            })
            .collect())
    }

    async fn discover_backend_monitoring_nodes(
        &self,
    ) -> Result<Vec<MonitoringBackendNode>, SearcherError> {
        match self.discover_backend_nodes_via_show_backends().await {
            Ok(nodes) if !nodes.is_empty() => Ok(nodes),
            Ok(_) | Err(_) => self.discover_backend_nodes_via_http().await,
        }
    }

    async fn build_fe_monitoring_result(
        &self,
        monitor_type: &str,
        priority: &str,
        include_raw_metrics: bool,
    ) -> serde_json::Value {
        let Some(http_url) = &self.http_url else {
            return serde_json::json!({
                "success": false,
                "node_type": "fe",
                "error": "Doris HTTP URL not configured"
            });
        };

        let url = format!("{}/metrics", http_url.trim_end_matches('/'));
        match self.fetch_metrics_from_url(&url).await {
            Ok(raw_metrics) => {
                let (definitions, _) =
                    Self::monitoring_definitions_for_scope("fe", monitor_type, priority);
                let scoped_metrics = if priority == "all" {
                    raw_metrics.clone()
                } else {
                    Self::filter_metrics_by_definitions(&raw_metrics, &definitions)
                };
                let dashboard_metrics = Self::calculate_fe_dashboard_metrics(&scoped_metrics);
                let summary = Self::calculate_monitoring_summary(&scoped_metrics, "fe");

                serde_json::json!({
                    "success": true,
                    "node_type": "fe",
                    "node_info": {
                        "host": self.host,
                        "source": "fe_http_url"
                    },
                    "url": url,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "metrics": if include_raw_metrics {
                        Self::to_json_value(&scoped_metrics)
                    } else {
                        Self::to_json_value(&dashboard_metrics)
                    },
                    "dashboard_metrics": Self::to_json_value(&dashboard_metrics),
                    "summary": summary,
                    "p0_metrics_info": if priority == "all" {
                        serde_json::Value::Null
                    } else {
                        Self::to_json_value(&definitions)
                    },
                    "raw_metrics": if include_raw_metrics {
                        Self::to_json_value(&scoped_metrics)
                    } else {
                        serde_json::Value::Null
                    }
                })
            }
            Err(error) => serde_json::json!({
                "success": false,
                "node_type": "fe",
                "url": url,
                "error": error.to_string()
            }),
        }
    }

    async fn build_be_monitoring_result(
        &self,
        monitor_type: &str,
        priority: &str,
        include_raw_metrics: bool,
    ) -> serde_json::Value {
        if monitor_type == "jvm" {
            return serde_json::json!([]);
        }

        let nodes = match self.discover_backend_monitoring_nodes().await {
            Ok(nodes) => nodes,
            Err(error) => {
                return serde_json::json!([{
                    "success": false,
                    "node_type": "be",
                    "error": error.to_string()
                }]);
            }
        };

        let (definitions, _) = Self::monitoring_definitions_for_scope("be", monitor_type, priority);
        let mut results = Vec::new();

        for node in nodes.into_iter().filter(|node| node.alive) {
            let url = format!("http://{}:{}/metrics", node.host, node.http_port);
            match self.fetch_metrics_from_url(&url).await {
                Ok(raw_metrics) => {
                    let scoped_metrics = if priority == "all" {
                        raw_metrics.clone()
                    } else {
                        Self::filter_metrics_by_definitions(&raw_metrics, &definitions)
                    };
                    let dashboard_metrics = Self::calculate_be_dashboard_metrics(&scoped_metrics);
                    let summary = Self::calculate_monitoring_summary(&scoped_metrics, "be");

                    results.push(serde_json::json!({
                        "success": true,
                        "node_type": "be",
                        "node_info": Self::to_json_value(&node),
                        "url": url,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                        "metrics": if include_raw_metrics {
                            Self::to_json_value(&scoped_metrics)
                        } else {
                            Self::to_json_value(&dashboard_metrics)
                        },
                        "dashboard_metrics": Self::to_json_value(&dashboard_metrics),
                        "summary": summary,
                        "p0_metrics_info": if priority == "all" {
                            serde_json::Value::Null
                        } else {
                            Self::to_json_value(&definitions)
                        },
                        "raw_metrics": if include_raw_metrics {
                            Self::to_json_value(&scoped_metrics)
                        } else {
                            serde_json::Value::Null
                        }
                    }));
                }
                Err(error) => {
                    results.push(serde_json::json!({
                        "success": false,
                        "node_type": "be",
                        "node_info": Self::to_json_value(&node),
                        "url": url,
                        "error": error.to_string()
                    }));
                }
            }
        }

        serde_json::Value::Array(results)
    }

    async fn build_monitoring_data_payload(
        &self,
        role: &str,
        monitor_type: &str,
        priority: &str,
        include_raw_metrics: bool,
    ) -> serde_json::Value {
        let mut data = serde_json::Map::new();
        if role == "fe" || role == "all" {
            data.insert(
                "fe".to_string(),
                self.build_fe_monitoring_result(monitor_type, priority, include_raw_metrics)
                    .await,
            );
        }
        if role == "be" || role == "all" {
            data.insert(
                "be".to_string(),
                self.build_be_monitoring_result(monitor_type, priority, include_raw_metrics)
                    .await,
            );
            if monitor_type == "jvm" {
                data.insert(
                    "be_jvm_info".to_string(),
                    serde_json::json!("BE nodes do not expose JVM metrics"),
                );
            }
        }
        serde_json::Value::Object(data)
    }

    fn build_monitoring_definitions_payload(
        role: &str,
        monitor_type: &str,
        priority: &str,
    ) -> (serde_json::Value, Option<String>) {
        let (definitions, note) = Self::monitoring_definitions_for_scope(role, monitor_type, priority);
        if priority == "core" {
            return (Self::to_json_value(&definitions), note);
        }

        let mut payload = serde_json::Map::new();
        if role == "fe" || role == "all" {
            if monitor_type == "process" {
                payload.insert("fe_process_p0_metrics".to_string(), Self::to_json_value(&definitions));
            } else if monitor_type == "jvm" {
                payload.insert("fe_jvm_p0_metrics".to_string(), Self::to_json_value(&definitions));
            } else if monitor_type == "machine" {
                payload.insert("fe_machine_p0_metrics".to_string(), Self::to_json_value(&definitions));
            } else {
                let (fe_definitions, _) = Self::monitoring_definitions_for_scope("fe", "all", priority);
                payload.insert("fe_p0_metrics".to_string(), Self::to_json_value(&fe_definitions));
            }
        }
        if role == "be" || role == "all" {
            if monitor_type == "process" {
                let (be_definitions, _) = Self::monitoring_definitions_for_scope("be", "process", priority);
                payload.insert("be_process_p0_metrics".to_string(), Self::to_json_value(&be_definitions));
            } else if monitor_type == "jvm" {
                payload.insert(
                    "be_jvm_info".to_string(),
                    serde_json::json!("BE nodes do not have JVM metrics"),
                );
            } else if monitor_type == "machine" {
                let (be_definitions, _) = Self::monitoring_definitions_for_scope("be", "machine", priority);
                payload.insert("be_machine_p0_metrics".to_string(), Self::to_json_value(&be_definitions));
            } else {
                let (be_definitions, _) = Self::monitoring_definitions_for_scope("be", "all", priority);
                payload.insert("be_p0_metrics".to_string(), Self::to_json_value(&be_definitions));
            }
        }

        (serde_json::Value::Object(payload), note)
    }

    /// Get Doris monitoring metrics definitions and/or sampled data.
    pub async fn get_monitoring_metrics(
        &self,
        content_type: Option<&str>,
        role: Option<&str>,
        monitor_type: Option<&str>,
        priority: Option<&str>,
        include_raw_metrics: bool,
    ) -> Result<MonitoringMetricsResponse, SearcherError> {
        let content_type = Self::normalize_choice(
            content_type,
            "data",
            &["definitions", "data", "both"],
            "content_type",
        )?;
        let role = Self::normalize_choice(role, "all", &["fe", "be", "all"], "role")?;
        let monitor_type = Self::normalize_choice(
            monitor_type,
            "all",
            &["process", "jvm", "machine", "all"],
            "monitor_type",
        )?;
        let priority =
            Self::normalize_choice(priority, "core", &["core", "p0", "all"], "priority")?;
        let timestamp = chrono::Utc::now().to_rfc3339();
        let include_definitions_in_error = content_type == "both";

        let (definitions, definitions_note) =
            Self::build_monitoring_definitions_payload(&role, &monitor_type, &priority);

        if content_type == "definitions" {
            return Ok(MonitoringMetricsResponse {
                success: true,
                content_type,
                role,
                monitor_type,
                priority,
                include_raw_metrics,
                format_type: "prometheus".to_string(),
                timestamp,
                definitions: Some(definitions),
                data: None,
                execution_info: None,
                note: definitions_note,
                error: None,
            });
        }

        if self.http_client.is_none() || self.http_url.is_none() {
            return Ok(MonitoringMetricsResponse {
                success: false,
                content_type,
                role,
                monitor_type,
                priority,
                include_raw_metrics,
                format_type: "prometheus".to_string(),
                timestamp,
                definitions: include_definitions_in_error.then_some(definitions),
                data: None,
                execution_info: None,
                note: definitions_note,
                error: Some("HTTP API URL not configured. Please set DORIS_HTTP_URL.".to_string()),
            });
        }

        let data = self
            .build_monitoring_data_payload(&role, &monitor_type, &priority, include_raw_metrics)
            .await;

        if content_type == "data" {
            return Ok(MonitoringMetricsResponse {
                success: true,
                content_type,
                role,
                monitor_type,
                priority,
                include_raw_metrics,
                format_type: "prometheus".to_string(),
                timestamp,
                definitions: None,
                data: Some(data),
                execution_info: None,
                note: definitions_note,
                error: None,
            });
        }

        Ok(MonitoringMetricsResponse {
            success: true,
            content_type,
            role,
            monitor_type,
            priority,
            include_raw_metrics,
            format_type: "prometheus".to_string(),
            timestamp,
            definitions: Some(definitions),
            data: Some(data),
            execution_info: Some(MonitoringMetricsExecutionInfo {
                combined_response: true,
                definitions_available: true,
                data_available: true,
            }),
            note: definitions_note,
            error: None,
        })
    }

    async fn get_query_id_by_trace_id(&self, trace_id: &str) -> Result<Option<String>, SearcherError> {
        let client = self
            .http_client
            .as_ref()
            .ok_or_else(|| SearcherError::ApiError("Doris HTTP client not configured".to_string()))?;
        let http_url = self
            .http_url
            .as_ref()
            .ok_or_else(|| SearcherError::ApiError("Doris HTTP URL not configured".to_string()))?;
        let url = format!(
            "{}/rest/v2/manager/query/trace_id/{}",
            http_url.trim_end_matches('/'),
            trace_id
        );

        for _ in 0..5 {
            let response = client
                .get(&url)
                .basic_auth(&self.username, Some(&self.password))
                .send()
                .await?;

            if response.status().is_success() {
                let body = response.text().await?;
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    if json.get("code").and_then(|value| value.as_i64()) == Some(0) {
                        let data = &json["data"];
                        if let Some(query_id) = data.as_str() {
                            return Ok(Some(query_id.to_string()));
                        }
                        if let Some(query_id) = data.get("query_id").and_then(|value| value.as_str()) {
                            return Ok(Some(query_id.to_string()));
                        }
                        if let Some(query_ids) = data.get("query_ids").and_then(|value| value.as_array()) {
                            if let Some(query_id) = query_ids.first().and_then(|value| value.as_str()) {
                                return Ok(Some(query_id.to_string()));
                            }
                        }
                    }
                } else {
                    let trimmed = body.trim();
                    if !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("null") {
                        return Ok(Some(trimmed.to_string()));
                    }
                }
            } else if !matches!(
                response.status(),
                reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::ACCEPTED
            ) {
                return Err(SearcherError::ApiError(format!(
                    "Failed to get query ID from trace ID: HTTP {}",
                    response.status()
                )));
            }

            sleep(Duration::from_secs(1)).await;
        }

        Ok(None)
    }

    async fn get_profile_by_query_id(
        &self,
        query_id: &str,
    ) -> Result<Option<(String, String, String)>, SearcherError> {
        let client = self
            .http_client
            .as_ref()
            .ok_or_else(|| SearcherError::ApiError("Doris HTTP client not configured".to_string()))?;
        let http_url = self
            .http_url
            .as_ref()
            .ok_or_else(|| SearcherError::ApiError("Doris HTTP URL not configured".to_string()))?;
        let urls = [
            format!(
                "{}/rest/v2/manager/query/profile/text/{}",
                http_url.trim_end_matches('/'),
                query_id
            ),
            format!(
                "{}/api/profile/text?query_id={}",
                http_url.trim_end_matches('/'),
                query_id
            ),
        ];

        for url in urls {
            let response = client
                .get(&url)
                .basic_auth(&self.username, Some(&self.password))
                .send()
                .await?;

            if response.status().is_success() {
                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_string();
                let body = response.text().await?;

                if content_type.contains("application/json") {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                        if json.get("code").and_then(|value| value.as_i64()) == Some(0) {
                            let data = &json["data"];
                            let profile_text = data
                                .get("profile")
                                .and_then(|value| value.as_str())
                                .or_else(|| data.as_str());

                            if let Some(profile_text) = profile_text {
                                return Ok(Some((
                                    profile_text.to_string(),
                                    url,
                                    chrono::Utc::now().to_rfc3339(),
                                )));
                            }
                        }
                    }
                } else if !body.trim().is_empty() && !body.to_ascii_lowercase().contains("not found") {
                    return Ok(Some((body, url, chrono::Utc::now().to_rfc3339())));
                }
            }
        }

        Ok(None)
    }

    /// Get FE (Frontend) nodes status
    pub async fn get_fe_status(&self) -> Result<FeStatusResponse, SearcherError> {
        let Some(http_url) = &self.http_url else {
            return Err(SearcherError::ApiError(
                "HTTP API URL not configured".to_string(),
            ));
        };

        let client = self.http_client.as_ref().unwrap();
        let url = format!("{}/api/bootstrap", http_url);

        let response = client.get(&url).send().await?;
        let json: serde_json::Value = response.json().await?;

        // Parse FE status from response
        Ok(FeStatusResponse {
            status: json["status"].as_str().unwrap_or("unknown").to_string(),
            name: json["name"].as_str().map(|s| s.to_string()),
        })
    }

    /// Get BE (Backend) nodes status
    pub async fn get_be_status(&self) -> Result<BeStatusResponse, SearcherError> {
        let Some(http_url) = &self.http_url else {
            return Err(SearcherError::ApiError(
                "HTTP API URL not configured".to_string(),
            ));
        };

        let client = self.http_client.as_ref().unwrap();
        let url = format!("{}/api/backends", http_url);

        let response = client.get(&url).send().await?;
        let json: serde_json::Value = response.json().await?;

        let backends: Vec<BackendNode> = json["backends"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| {
                Some(BackendNode {
                    name: v["name"].as_str()?.to_string(),
                    host: v["host"].as_str()?.to_string(),
                    heartbeat_port: v["heartbeat_port"].as_u64()? as u16,
                    alive: v["alive"].as_bool().unwrap_or(false),
                    capacity: v["capacity"].as_str().map(|s| s.to_string()),
                })
            })
            .collect();

        Ok(BeStatusResponse {
            total: backends.len(),
            alive: backends.iter().filter(|b| b.alive).count(),
            backends,
        })
    }

    /// Get query statistics
    pub async fn get_query_stats(&self) -> Result<QueryStatsResponse, SearcherError> {
        let sql = "SHOW QUERY STATS";
        let result = self.execute_query(sql).await?;

        let stats: Vec<QueryStat> = result
            .data
            .iter()
            .filter_map(|row| {
                Some(QueryStat {
                    table_name: row.get("TableName")?.as_str()?.to_string(),
                    query_count: row.get("QueryCount")?.as_u64().unwrap_or(0),
                })
            })
            .collect();

        Ok(QueryStatsResponse {
            total: stats.len(),
            stats,
        })
    }

    /// Get routine load jobs
    pub async fn get_routine_loads(&self) -> Result<RoutineLoadResponse, SearcherError> {
        let sql = "SHOW ROUTINE LOAD";
        tracing::info!("Getting routine load jobs with SQL: {}", sql);

        let result = self.execute_query(sql).await?;
        tracing::info!(
            "Query executed, returned {} rows, columns: {:?}",
            result.row_count,
            result.columns
        );

        // Log raw data for debugging
        if !result.data.is_empty() {
            tracing::debug!("First row data: {:?}", result.data[0]);
        } else {
            tracing::warn!("No routine load data returned from query");
        }
        let jobs: Vec<RoutineLoadJob> = result
            .data
            .iter()
            .filter_map(|row| {
                tracing::debug!("Processing row: {:?}", row);

                let id = row.get("Id").and_then(|v| v.as_str());
                let name = row.get("Name").and_then(|v| v.as_str());
                let db = row.get("DbName").or_else(|| row.get("Db")).and_then(|v| v.as_str());
                let table = row.get("TableName").and_then(|v| v.as_str());
                let state = row.get("State").and_then(|v| v.as_str());

                if let (Some(id), Some(name), Some(db), Some(table), Some(state)) = (id, name, db, table, state) {
                    tracing::debug!(
                        "Parsed routine load: id={}, name={}, db={}, table={}, state={}",
                        id, name, db, table, state
                    );
                    Some(RoutineLoadJob {
                        id: id.to_string(),
                        name: name.to_string(),
                        database: db.to_string(),
                        table: table.to_string(),
                        state: state.to_string(),
                        create_time: row.get("CreateTime").and_then(|v| v.as_str().map(|s| s.to_string())),
                        end_time: row.get("EndTime").and_then(|v| v.as_str().map(|s| s.to_string())),
                        pause_time: row.get("PauseTime").and_then(|v| v.as_str().map(|s| s.to_string())),
                        data_source_type: row.get("DataSourceType").and_then(|v| v.as_str().map(|s| s.to_string())),
                        data_source_properties: row.get("DataSourceProperties").and_then(|v| v.as_str().map(|s| s.to_string())),
                        custom_properties: row.get("CustomProperties").and_then(|v| v.as_str().map(|s| s.to_string())),
                        statistic: row.get("Statistic").and_then(|v| v.as_str().map(|s| s.to_string())),
                        progress: row.get("Progress").and_then(|v| v.as_str().map(|s| s.to_string())),
                        lag: row.get("Lag").and_then(|v| v.as_str().map(|s| s.to_string())),
                        job_properties: row.get("JobProperties").and_then(|v| v.as_str().map(|s| s.to_string())),
                        error_log_urls: row.get("ErrorLogUrls").and_then(|v| v.as_str().map(|s| s.to_string())),
                        other_msg: row.get("OtherMsg").and_then(|v| v.as_str().map(|s| s.to_string())),
                        reason_of_state_changed: row.get("ReasonOfStateChanged").and_then(|v| v.as_str().map(|s| s.to_string())),
                        is_multi_table: row.get("IsMultiTable").and_then(|v| v.as_str().map(|s| s.to_string())),
                        current_task_num: row.get("CurrentTaskNum").and_then(|v| v.as_str().map(|s| s.to_string())),
                        user: row.get("User").and_then(|v| v.as_str().map(|s| s.to_string())),
                        comment: row.get("Comment").and_then(|v| v.as_str().map(|s| s.to_string())),
                    })
                } else {
                    tracing::warn!(
                        "Failed to parse row, got values - Id: {:?}, Name: {:?}, DbName: {:?}, TableName: {:?}, State: {:?}",
                        row.get("Id"),
                        row.get("Name"),
                        row.get("DbName").or_else(|| row.get("Db")),
                        row.get("TableName"),
                        row.get("State")
                    );
                    None
                }
            })
            .collect();

        tracing::info!("Successfully parsed {} routine load jobs", jobs.len());

        Ok(RoutineLoadResponse {
            total: jobs.len(),
            jobs,
        })
    }

    /// Get load jobs
    pub async fn get_load_jobs(&self) -> Result<LoadJobResponse, SearcherError> {
        let sql = "SHOW LOAD";
        let result = self.execute_query(sql).await?;

        let jobs: Vec<LoadJob> = result
            .data
            .iter()
            .filter_map(|row| {
                Some(LoadJob {
                    job_id: row.get("JobId")?.as_str()?.to_string(),
                    label: row.get("Label")?.as_str()?.to_string(),
                    state: row.get("State")?.as_str()?.to_string(),
                    progress: row.get("Progress").and_then(|v| v.as_str().map(|s| s.to_string())),
                    load_type: row.get("Type").and_then(|v| v.as_str().map(|s| s.to_string())),
                    create_time: row.get("CreateTime").and_then(|v| v.as_str().map(|s| s.to_string())),
                })
            })
            .collect();

        Ok(LoadJobResponse {
            total: jobs.len(),
            jobs,
        })
    }
}

// Response types

/// Query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub data: Vec<HashMap<String, serde_json::Value>>,
    pub columns: Vec<String>,
    pub row_count: usize,
    pub total_row_count: usize,
    pub truncated: bool,
    pub execution_time_ms: u64,
    pub sql: String,
}

/// Database list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseListResponse {
    pub catalog_name: Option<String>,
    pub databases: Vec<String>,
    pub count: usize,
}

/// Table list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableListResponse {
    pub catalog_name: Option<String>,
    pub database: String,
    pub tables: Vec<String>,
    pub count: usize,
}

/// Column schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: String,
    pub is_nullable: String,
    pub default_value: Option<String>,
    pub comment: Option<String>,
}

/// Table schema response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchemaResponse {
    pub catalog_name: Option<String>,
    pub database: String,
    pub table: String,
    pub columns: Vec<ColumnSchema>,
    pub column_count: usize,
}

/// Table metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableMetadata {
    pub catalog_name: Option<String>,
    pub database: String,
    pub table: String,
    pub row_count: Option<u64>,
    pub data_length: Option<u64>,
    pub index_length: Option<u64>,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
}

/// Basic table column information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableBasicColumnInfo {
    pub column_name: String,
    pub data_type: String,
    pub nullable: bool,
    pub default_value: Option<String>,
    pub column_comment: Option<String>,
}

/// Table partition information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TablePartitionInfo {
    pub partition_name: String,
    pub partition_description: Option<String>,
    pub table_rows: Option<u64>,
    pub data_length: Option<u64>,
    pub index_length: Option<u64>,
}

/// Table partitions wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TablePartitionsInfo {
    pub partition_count: usize,
    pub partitions: Vec<TablePartitionInfo>,
}

/// Table size information used by analytics helpers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSizeInfo {
    pub engine: Option<String>,
    pub estimated_rows: Option<u64>,
    pub data_length: Option<u64>,
    pub index_length: Option<u64>,
    pub total_size: Option<u64>,
    pub total_size_formatted: Option<String>,
}

/// Table basic information response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableBasicInfoResponse {
    pub table_name: String,
    pub catalog_name: String,
    pub database: String,
    pub analysis_timestamp: String,
    pub row_count: u64,
    pub column_count: usize,
    pub columns_info: Vec<TableBasicColumnInfo>,
    pub partitions_info: TablePartitionsInfo,
    pub table_size: TableSizeInfo,
    pub execution_time_seconds: f64,
}

/// Table comment response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableCommentResponse {
    pub catalog_name: Option<String>,
    pub database: String,
    pub table: String,
    pub comment: Option<String>,
}

/// Catalog information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogInfo {
    pub catalog_id: Option<String>,
    pub catalog_name: String,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    pub is_current: Option<String>,
    pub create_time: Option<String>,
    pub last_update_time: Option<String>,
    pub comment: Option<String>,
}

/// Catalog list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogListResponse {
    pub catalogs: Vec<CatalogInfo>,
    pub count: usize,
}

/// SQL result summary for profile output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlResultSummary {
    pub row_count: usize,
    pub returned_row_count: usize,
    pub truncated: bool,
    pub columns: Vec<String>,
}

/// SQL profile response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlProfileResponse {
    pub success: bool,
    pub trace_id: String,
    pub query_id: Option<String>,
    pub sql: String,
    pub database: String,
    pub catalog_name: String,
    pub execution_time_ms: u64,
    pub sql_result_summary: SqlResultSummary,
    pub profile_text: Option<String>,
    pub profile_endpoint: Option<String>,
    pub retrieved_at: Option<String>,
    pub error: Option<String>,
}

/// Recent audit log response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogResponse {
    pub days: u32,
    pub limit: usize,
    pub since_date: String,
    pub count: usize,
    pub logs: Vec<HashMap<String, serde_json::Value>>,
}

/// Filters used for table data size queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDataSizeFilters {
    pub db_name: Option<String>,
    pub table_name: Option<String>,
}

/// Table-level table data size record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDataSizeTable {
    pub table_name: String,
    pub size_bytes: u64,
    pub size_formatted: String,
    pub replica_count: Option<u64>,
    pub details: serde_json::Value,
}

/// Database-level table data size aggregation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDataSizeDatabase {
    pub database_name: String,
    pub table_count: usize,
    pub total_size_bytes: u64,
    pub total_size_formatted: String,
    pub tables: BTreeMap<String, TableDataSizeTable>,
}

/// Aggregated table data size payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDataSizeReport {
    pub summary: TableDataSizeSummary,
    pub databases: BTreeMap<String, TableDataSizeDatabase>,
}

/// Summary for table data size query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDataSizeSummary {
    pub total_databases: usize,
    pub total_tables: usize,
    pub total_size_bytes: u64,
    pub total_size_formatted: String,
    pub single_replica: bool,
    pub query_filters: TableDataSizeFilters,
}

/// Table data size response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableDataSizeResponse {
    pub success: bool,
    pub db_name: Option<String>,
    pub table_name: Option<String>,
    pub single_replica: bool,
    pub timestamp: String,
    pub data: Option<TableDataSizeReport>,
    pub url: String,
    pub note: Option<String>,
    pub error: Option<String>,
    pub raw_response_preview: Option<String>,
}

/// Realtime memory stats response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeMemoryStats {
    pub success: bool,
    pub tracker_type: String,
    pub include_details: bool,
    pub timestamp: String,
    pub memory_stats: serde_json::Value,
    pub note: Option<String>,
    pub error: Option<String>,
}

/// Historical memory stats response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalMemoryStats {
    pub success: bool,
    pub tracker_names: Option<Vec<String>>,
    pub time_range: String,
    pub timestamp: String,
    pub historical_stats: serde_json::Value,
    pub note: Option<String>,
    pub error: Option<String>,
}

/// Combined memory stats execution info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStatsExecutionInfo {
    pub combined_response: bool,
    pub realtime_available: bool,
    pub historical_available: bool,
}

/// Doris memory stats response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStatsResponse {
    pub success: bool,
    pub data_type: String,
    pub timestamp: String,
    pub realtime: Option<RealtimeMemoryStats>,
    pub historical: Option<HistoricalMemoryStats>,
    pub execution_info: Option<MemoryStatsExecutionInfo>,
    pub error: Option<String>,
}

/// Monitoring metric definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringMetricDefinition {
    pub name: String,
    pub meaning: String,
    pub description: String,
    pub unit: String,
}

/// Prometheus metric sample with labels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringMetricSample {
    pub labels: BTreeMap<String, String>,
    pub value: f64,
}

/// Parsed Prometheus metric value
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MonitoringMetricValue {
    Number(f64),
    Samples(Vec<MonitoringMetricSample>),
}

/// Minimal backend node info used by monitoring tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringBackendNode {
    pub backend_id: Option<String>,
    pub host: String,
    pub http_port: u16,
    pub alive: bool,
    pub source: String,
}

/// Monitoring tool execution info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringMetricsExecutionInfo {
    pub combined_response: bool,
    pub definitions_available: bool,
    pub data_available: bool,
}

/// Doris monitoring metrics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringMetricsResponse {
    pub success: bool,
    pub content_type: String,
    pub role: String,
    pub monitor_type: String,
    pub priority: String,
    pub include_raw_metrics: bool,
    pub format_type: String,
    pub timestamp: String,
    pub definitions: Option<serde_json::Value>,
    pub data: Option<serde_json::Value>,
    pub execution_info: Option<MonitoringMetricsExecutionInfo>,
    pub note: Option<String>,
    pub error: Option<String>,
}

/// FE status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeStatusResponse {
    pub status: String,
    pub name: Option<String>,
}

/// Backend node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendNode {
    pub name: String,
    pub host: String,
    pub heartbeat_port: u16,
    pub alive: bool,
    pub capacity: Option<String>,
}

/// BE status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeStatusResponse {
    pub total: usize,
    pub alive: usize,
    pub backends: Vec<BackendNode>,
}

/// Query statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryStat {
    pub table_name: String,
    pub query_count: u64,
}

/// Query statistics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryStatsResponse {
    pub total: usize,
    pub stats: Vec<QueryStat>,
}

/// Routine load job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineLoadJob {
    pub id: String,
    pub name: String,
    pub database: String,
    pub table: String,
    pub state: String,
    pub create_time: Option<String>,
    pub end_time: Option<String>,
    pub pause_time: Option<String>,
    pub data_source_type: Option<String>,
    pub data_source_properties: Option<String>,
    pub custom_properties: Option<String>,
    pub statistic: Option<String>,
    pub progress: Option<String>,
    pub lag: Option<String>,
    pub job_properties: Option<String>,
    pub error_log_urls: Option<String>,
    pub other_msg: Option<String>,
    pub reason_of_state_changed: Option<String>,
    pub is_multi_table: Option<String>,
    pub current_task_num: Option<String>,
    pub user: Option<String>,
    pub comment: Option<String>,
}

/// Routine load response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineLoadResponse {
    pub total: usize,
    pub jobs: Vec<RoutineLoadJob>,
}

/// Load job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadJob {
    pub job_id: String,
    pub label: String,
    pub state: String,
    pub progress: Option<String>,
    pub load_type: Option<String>,
    pub create_time: Option<String>,
}

/// Load job response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadJobResponse {
    pub total: usize,
    pub jobs: Vec<LoadJob>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn init() {
        let _ = tracing_subscriber::fmt().try_init();
    }

    #[test]
    fn test_extract_table_references_from_sql_handles_common_shapes() {
        let refs = DorisClient::extract_table_references_from_sql(
            "INSERT INTO orders_summary SELECT * FROM orders o JOIN dim.users u ON o.user_id = u.id",
            Some("internal"),
            Some("analytics"),
        );

        assert_eq!(
            refs,
            vec![
                "internal.analytics.orders".to_string(),
                "internal.analytics.orders_summary".to_string(),
                "internal.dim.users".to_string(),
            ]
        );

        let qualified_refs = DorisClient::extract_table_references_from_sql(
            "SELECT * FROM lakehouse.sales.orders JOIN internal.analytics.users ON orders.user_id = users.id",
            Some("internal"),
            Some("analytics"),
        );

        assert_eq!(
            qualified_refs,
            vec![
                "internal.analytics.users".to_string(),
                "lakehouse.sales.orders".to_string(),
            ]
        );
    }

    #[test]
    fn test_extract_destination_table_from_sql_handles_insert_and_create() {
        let insert_target = DorisClient::extract_destination_table_from_sql(
            "INSERT INTO fact_orders SELECT * FROM ods_orders",
            Some("internal"),
            Some("warehouse"),
        );
        assert_eq!(
            insert_target,
            Some("internal.warehouse.fact_orders".to_string())
        );

        let create_target = DorisClient::extract_destination_table_from_sql(
            "CREATE TABLE mart.daily_orders AS SELECT * FROM warehouse.fact_orders",
            Some("internal"),
            Some("warehouse"),
        );
        assert_eq!(create_target, Some("internal.mart.daily_orders".to_string()));
    }

    #[test]
    fn test_simplify_sql_pattern_normalizes_literals() {
        let pattern = DorisClient::simplify_sql_pattern(
            "select * from users where id = 123 and city = 'shanghai' limit 10",
        );

        assert_eq!(
            pattern,
            "SELECT * FROM USERS WHERE ID = ? AND CITY = ? LIMIT ?"
        );
    }

    #[tokio::test]
    async fn test_doris_routine_load() {
        init();
        // Load environment variables from .env file
        dotenv::dotenv().ok();

        // Get configuration from environment variables
        let host =
            std::env::var("DORIS_HOST").expect("DORIS_HOST environment variable must be set");
        let port = std::env::var("DORIS_PORT").unwrap_or_else(|_| "9030".to_string());
        let username = std::env::var("DORIS_USERNAME")
            .expect("DORIS_USERNAME environment variable must be set");
        let password = std::env::var("DORIS_PASSWORD")
            .expect("DORIS_PASSWORD environment variable must be set");
        let database =
            std::env::var("DORIS_DB").expect("DORIS_DB environment variable must be set");
        let http_url = std::env::var("DORIS_HTTP_URL").ok();

        println!("Creating Doris client...");
        println!("  Host: {}", host);
        println!("  Port: {}", port);
        println!("  Username: {}", username);
        println!("  Database: {}", database);
        println!("  HTTP URL: {:?}", http_url);

        // Create Doris client
        let client = DorisClient::new(host, port, username, password, database, http_url);

        // Test: Execute SHOW ROUTINE LOAD
        println!("\nExecuting: SHOW ROUTINE LOAD");
        match client.get_routine_loads().await {
            Ok(response) => {
                println!("✓ Successfully executed SHOW ROUTINE LOAD");
                println!("  Total jobs: {}", response.total);

                if response.total > 0 {
                    println!("  Jobs:");
                    for (i, job) in response.jobs.iter().enumerate() {
                        println!("    {}. Name: {}", i + 1, job.name);
                        println!("       Database: {}", job.database);
                        println!("       Table: {}", job.table);
                        println!("       State: {}", job.state);
                    }
                } else {
                    println!("  No routine load jobs found");
                }

                // Verify the response
                assert_eq!(
                    response.total,
                    response.jobs.len(),
                    "Total count should match the actual number of jobs"
                );
            }
            Err(e) => {
                eprintln!("✗ Failed to execute SHOW ROUTINE LOAD: {}", e);
                panic!("Test failed: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_doris_execute_sql() {
        init();
        // Load environment variables from .env file
        dotenv::dotenv().ok();

        // Get configuration from environment variables
        let host =
            std::env::var("DORIS_HOST").expect("DORIS_HOST environment variable must be set");
        let port = std::env::var("DORIS_PORT").unwrap_or_else(|_| "9030".to_string());
        let username = std::env::var("DORIS_USERNAME")
            .expect("DORIS_USERNAME environment variable must be set");
        let password = std::env::var("DORIS_PASSWORD")
            .expect("DORIS_PASSWORD environment variable must be set");
        let database =
            std::env::var("DORIS_DB").expect("DORIS_DB environment variable must be set");
        let http_url = std::env::var("DORIS_HTTP_URL").ok();

        println!("Creating Doris client...");
        let client = DorisClient::new(host, port, username, password, database, http_url);

        // Test: Execute SHOW DATABASES
        println!("\nExecuting: SHOW DATABASES");
        match client.get_databases().await {
            Ok(response) => {
                println!("✓ Successfully executed SHOW DATABASES");
                println!("  Total databases: {}", response.count);
                println!("  Databases: {:?}", response.databases);

                // Verify the response
                assert!(
                    !response.databases.is_empty(),
                    "Should have at least one database"
                );
                assert_eq!(
                    response.count,
                    response.databases.len(),
                    "Count should match the actual number of databases"
                );
            }
            Err(e) => {
                eprintln!("✗ Failed to execute SHOW DATABASES: {}", e);
                panic!("Test failed: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_doris_all_methods() {
        init();
        dotenv::dotenv().ok();

        let host = std::env::var("DORIS_HOST").expect("DORIS_HOST must be set");
        let port = std::env::var("DORIS_PORT").unwrap_or_else(|_| "9030".to_string());
        let username = std::env::var("DORIS_USERNAME").expect("DORIS_USERNAME must be set");
        let password = std::env::var("DORIS_PASSWORD").expect("DORIS_PASSWORD must be set");
        let database = std::env::var("DORIS_DB").expect("DORIS_DB must be set");
        let http_url = std::env::var("DORIS_HTTP_URL").ok();

        let client = DorisClient::new(host, port, username, password, database, http_url);

        println!("\n=== Testing All Doris Methods ===\n");

        // Test 1: get_databases
        println!("1. Testing get_databases()...");
        match client.get_databases().await {
            Ok(response) => println!("   ✓ Found {} databases", response.count),
            Err(e) => println!("   ✗ Failed: {}", e),
        }

        // Test 2: get_tables
        println!("2. Testing get_tables()...");
        match client.get_tables(&std::env::var("DORIS_DB").unwrap()).await {
            Ok(response) => println!("   ✓ Found {} tables", response.count),
            Err(e) => println!("   ✗ Failed: {}", e),
        }

        // Test 3: get_table_schema (use first table from get_tables)
        println!("3. Testing get_table_schema()...");
        match client.get_tables(&std::env::var("DORIS_DB").unwrap()).await {
            Ok(tables_resp) => {
                if let Some(first_table) = tables_resp.tables.first() {
                    match client.get_table_schema(&std::env::var("DORIS_DB").unwrap(), first_table).await {
                        Ok(schema) => println!("   ✓ Table {} has {} columns", first_table, schema.column_count),
                        Err(e) => println!("   ✗ Failed: {}", e),
                    }
                } else {
                    println!("   ⊘ No tables found to test schema");
                }
            }
            Err(e) => println!("   ✗ Failed to get tables: {}", e),
        }

        // Test 4: get_table_metadata
        println!("4. Testing get_table_metadata()...");
        match client.get_tables(&std::env::var("DORIS_DB").unwrap()).await {
            Ok(tables_resp) => {
                if let Some(first_table) = tables_resp.tables.first() {
                    match client.get_table_metadata(&std::env::var("DORIS_DB").unwrap(), first_table).await {
                        Ok(metadata) => println!("   ✓ Table {} metadata: {} rows", first_table, metadata.row_count.unwrap_or(0)),
                        Err(e) => println!("   ✗ Failed: {}", e),
                    }
                }
            }
            Err(e) => println!("   ✗ Failed: {}", e),
        }

        // Test 5: get_fe_status
        println!("5. Testing get_fe_status()...");
        match client.get_fe_status().await {
            Ok(response) => println!("   ✓ FE status: {}, name: {:?}", response.status, response.name),
            Err(e) => println!("   ✗ Failed: {}", e),
        }

        // Test 6: get_be_status
        println!("6. Testing get_be_status()...");
        match client.get_be_status().await {
            Ok(response) => println!("   ✓ BE status: {}/{} alive", response.alive, response.total),
            Err(e) => println!("   ✗ Failed: {}", e),
        }

        // Test 7: get_query_stats
        println!("7. Testing get_query_stats()...");
        match client.execute_query("SHOW QUERY STATS").await {
            Ok(result) => {
                println!("      Raw result: {} rows, columns: {:?}", result.row_count, result.columns);
                if let Some(first_row) = result.data.first() {
                    println!("      First row keys: {:?}", first_row.keys().collect::<Vec<_>>());
                }
            }
            Err(e) => println!("   ✗ Failed: {}", e),
        }
        match client.get_query_stats().await {
            Ok(response) => println!("   ✓ Found {} query stats", response.total),
            Err(e) => println!("   ✗ Failed: {}", e),
        }

        // Test 8: get_routine_loads
        println!("8. Testing get_routine_loads()...");
        match client.get_routine_loads().await {
            Ok(response) => println!("   ✓ Found {} routine load jobs", response.total),
            Err(e) => println!("   ✗ Failed: {}", e),
        }

        // Test 9: get_load_jobs
        println!("9. Testing get_load_jobs()...");
        match client.execute_query(&format!("SHOW LOAD FROM {} LIMIT 1", std::env::var("DORIS_DB").unwrap())).await {
            Ok(result) => {
                println!("      Raw SHOW LOAD result (limited): {} rows, columns: {:?}", result.row_count, result.columns);
                if let Some(first_row) = result.data.first() {
                    println!("      First row keys: {:?}", first_row.keys().collect::<Vec<_>>());
                }
            }
            Err(e) => println!("   ✗ Failed: {}", e),
        }
        match client.get_load_jobs().await {
            Ok(response) => println!("   ✓ Found {} load jobs", response.total),
            Err(e) => println!("   ✗ Failed: {}", e),
        }

        // Test 10: execute_query (custom SQL)
        println!("10. Testing execute_query() with SELECT 1...");
        match client.execute_query("SELECT 1 as test_column").await {
            Ok(result) => println!("   ✓ Query returned: {:?}", result.data.first()),
            Err(e) => println!("   ✗ Failed: {}", e),
        }

        println!("\n=== All Tests Completed ===\n");
    }

    #[test]
    fn test_sanitize_query_sql_allows_read_only_statements() {
        assert_eq!(
            DorisClient::sanitize_query_sql("SHOW LOAD;").unwrap(),
            "SHOW LOAD"
        );
        assert_eq!(
            DorisClient::sanitize_query_sql("EXPLAIN SELECT 1").unwrap(),
            "EXPLAIN SELECT 1"
        );
        assert_eq!(
            DorisClient::sanitize_query_sql("EXPLAIN ANALYZE SELECT * FROM test_table").unwrap(),
            "EXPLAIN ANALYZE SELECT * FROM test_table"
        );
    }

    #[test]
    fn test_sanitize_query_sql_rejects_unsafe_statements() {
        assert!(DorisClient::sanitize_query_sql("DROP TABLE t").is_err());
        assert!(DorisClient::sanitize_query_sql("SELECT 1; DROP TABLE t").is_err());
        assert!(DorisClient::sanitize_query_sql("EXPLAIN DELETE FROM t").is_err());
    }

    #[test]
    fn test_build_table_data_size_report_from_array_payload() {
        let payload = json!([
            {
                "database": "db1",
                "table": "table_a",
                "size": 1024,
                "replica_count": 3
            },
            {
                "database": "db1",
                "table": "table_b",
                "size": "2048"
            },
            {
                "database": "db2",
                "table": "table_c",
                "size": 512
            }
        ]);

        let report = DorisClient::build_table_data_size_report(&payload, None, None, false);

        assert_eq!(report.summary.total_databases, 2);
        assert_eq!(report.summary.total_tables, 3);
        assert_eq!(report.summary.total_size_bytes, 3584);
        assert_eq!(report.summary.total_size_formatted, "3.50 KB");
        assert_eq!(
            report.databases.get("db1").unwrap().total_size_formatted,
            "3.00 KB"
        );
        assert_eq!(
            report
                .databases
                .get("db1")
                .unwrap()
                .tables
                .get("table_a")
                .unwrap()
                .replica_count,
            Some(3)
        );
    }

    #[test]
    fn test_parse_prometheus_metrics_supports_simple_and_labeled_samples() {
        let metrics = DorisClient::parse_prometheus_metrics(
            r#"
            # HELP doris_be_cpu CPU metrics
            doris_be_memory_allocated_bytes 1073741824
            doris_be_cpu{device="cpu",mode="idle"} 80
            doris_be_cpu{device="cpu",mode="user"} 20
            doris_be_network_receive_bytes{device="eth0"} 1024
            doris_be_network_receive_bytes{device="lo"} 256
            "#,
        );

        match metrics.get("doris_be_memory_allocated_bytes").unwrap() {
            MonitoringMetricValue::Number(value) => assert_eq!(*value, 1073741824.0),
            _ => panic!("expected scalar metric"),
        }

        match metrics.get("doris_be_cpu").unwrap() {
            MonitoringMetricValue::Samples(samples) => assert_eq!(samples.len(), 2),
            _ => panic!("expected labeled metric series"),
        }

        assert_eq!(
            DorisClient::aggregate_network_bytes(&metrics, "doris_be_network_receive_bytes"),
            Some(1024.0)
        );
    }

    #[test]
    fn test_calculate_be_dashboard_metrics_extracts_expected_fields() {
        let metrics = DorisClient::parse_prometheus_metrics(
            r#"
            doris_be_cpu{device="cpu",mode="idle"} 70
            doris_be_cpu{device="cpu",mode="user"} 20
            doris_be_cpu{device="cpu",mode="system"} 10
            doris_be_disks_local_used_capacity 2147483648
            doris_be_disks_total_capacity 4294967296
            doris_be_memory_allocated_bytes 1073741824
            doris_be_process_fd_num_used 100
            doris_be_process_fd_num_limit_soft 1000
            doris_be_network_receive_bytes{device="eth0"} 4096
            doris_be_network_send_bytes{device="eth0"} 2048
            "#,
        );

        let dashboard = DorisClient::calculate_be_dashboard_metrics(&metrics);

        assert_eq!(dashboard.get("cpu_usage_percent"), Some(&30.0));
        assert_eq!(dashboard.get("disks_usage_percent"), Some(&50.0));
        assert_eq!(dashboard.get("process_fd_usage_percent"), Some(&10.0));
        assert_eq!(dashboard.get("network_receive_bytes_total"), Some(&4096.0));
        assert_eq!(dashboard.get("memory_allocated_bytes"), Some(&1073741824.0));
    }
}
