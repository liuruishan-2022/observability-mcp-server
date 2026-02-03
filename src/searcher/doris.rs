//! Doris searcher for Apache Doris database
//!
//! Provides MCP tools for querying Apache Doris database

use super::SearcherError;
use serde::{Deserialize, Serialize};
use sqlx::{Column, Row, mysql::MySqlRow};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

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

    /// Get a clone of the pool Arc, initializing if necessary
    async fn get_pool(&self) -> Result<Arc<Mutex<Option<sqlx::MySqlPool>>>, SearcherError> {
        self.ensure_connected().await?;
        Ok(Arc::clone(&self.pool))
    }

    /// Execute a SQL query and return results
    pub async fn execute_query(&self, sql: &str) -> Result<QueryResult, SearcherError> {
        tracing::info!("Executing Doris query: {}", sql);

        let start = std::time::Instant::now();

        // Get the pool (initializing if necessary)
        let pool_arc = self.get_pool().await?;
        let pool_guard = pool_arc.lock().await;
        let pool = pool_guard.as_ref().unwrap();

        // Execute the query using raw_sql for better Doris compatibility
        let result = sqlx::raw_sql(sql).fetch_all(pool).await.map_err(|e| {
            tracing::error!("Query execution failed: {}", e);
            SearcherError::ApiError(format!("Query execution failed: {}", e))
        })?;

        let execution_time_ms = start.elapsed().as_millis() as u64;

        // If no results, return empty result
        if result.is_empty() {
            tracing::info!("Query returned no results");
            return Ok(QueryResult {
                data: vec![],
                columns: vec![],
                row_count: 0,
                execution_time_ms,
                sql: sql.to_string(),
            });
        }

        // Get column names from the first row
        let columns: Vec<String> = result[0]
            .columns()
            .iter()
            .map(|col| col.name().to_string())
            .collect();

        tracing::debug!("Query columns: {:?}", columns);

        // Convert rows to HashMap format
        let data: Vec<std::collections::HashMap<String, serde_json::Value>> = result
            .iter()
            .map(|row| {
                let mut map = std::collections::HashMap::new();
                for (i, col) in row.columns().iter().enumerate() {
                    let col_name = col.name();
                    let value = Self::column_to_json(row, i);
                    map.insert(col_name.to_string(), value);
                }
                map
            })
            .collect();

        let row_count = data.len();
        tracing::info!(
            "Query returned {} rows in {}ms",
            row_count,
            execution_time_ms
        );

        Ok(QueryResult {
            data,
            columns,
            row_count,
            execution_time_ms,
            sql: sql.to_string(),
        })
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
        let result = self.execute_query("SHOW DATABASES").await?;
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
        Ok(DatabaseListResponse { databases, count })
    }

    /// Get list of tables in a database
    pub async fn get_tables(&self, database: &str) -> Result<TableListResponse, SearcherError> {
        let sql = format!("SHOW TABLES FROM `{}`", database);
        let result = self.execute_query(&sql).await?;
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
            database: database.to_string(),
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
        let sql = format!(
            "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE, COLUMN_DEFAULT, COLUMN_COMMENT
             FROM information_schema.COLUMNS
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}'
             ORDER BY ORDINAL_POSITION",
            database, table
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
            database: database.to_string(),
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
        // Query table size and row count from information_schema
        let sql = format!(
            "SELECT TABLE_ROWS, DATA_LENGTH, INDEX_LENGTH, CREATE_TIME, UPDATE_TIME
             FROM information_schema.TABLES
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}'",
            database, table
        );

        let result = self.execute_query(&sql).await?;

        if let Some(row) = result.data.first() {
            Ok(TableMetadata {
                database: database.to_string(),
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
                database, table
            )))
        }
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
                    query_id: row.get("QueryId")?.as_str()?.to_string(),
                    user: row.get("User")?.as_str()?.to_string(),
                    database: row.get("Db")?.as_str()?.to_string(),
                    state: row.get("State")?.as_str()?.to_string(),
                    duration_ms: row.get("DurationMs").and_then(|v| v.as_u64()),
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
                    database: row.get("Db")?.as_str()?.to_string(),
                    table: row.get("Table")?.as_str()?.to_string(),
                    state: row.get("State")?.as_str()?.to_string(),
                    label: row.get("Label")?.as_str()?.to_string(),
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
    pub execution_time_ms: u64,
    pub sql: String,
}

/// Database list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseListResponse {
    pub databases: Vec<String>,
    pub count: usize,
}

/// Table list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableListResponse {
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
    pub database: String,
    pub table: String,
    pub columns: Vec<ColumnSchema>,
    pub column_count: usize,
}

/// Table metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableMetadata {
    pub database: String,
    pub table: String,
    pub row_count: Option<u64>,
    pub data_length: Option<u64>,
    pub index_length: Option<u64>,
    pub create_time: Option<String>,
    pub update_time: Option<String>,
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
    pub query_id: String,
    pub user: String,
    pub database: String,
    pub state: String,
    pub duration_ms: Option<u64>,
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
    pub database: String,
    pub table: String,
    pub state: String,
    pub label: String,
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

    fn init() {
        let _ = tracing_subscriber::fmt().try_init();
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
}
