#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_doris_get_routine_loads() {
        // 从环境变量加载配置
        dotenv::dotenv().ok();

        let host = std::env::var("DORIS_HOST")
            .expect("DORIS_HOST must be set");
        let port = std::env::var("DORIS_PORT")
            .unwrap_or_else(|_| "9030".to_string());
        let username = std::env::var("DORIS_USERNAME")
            .expect("DORIS_USERNAME must be set");
        let password = std::env::var("DORIS_PASSWORD")
            .expect("DORIS_PASSWORD must be set");
        let database = std::env::var("DORIS_DB")
            .expect("DORIS_DB must be set");
        let http_url = std::env::var("DORIS_HTTP_URL").ok();

        println!("Creating Doris client with {}:{}@{}:{}", username, database, host, port);

        let client = DorisClient::new(host, port, username, password, database, http_url);

        // 测试获取 Routine Load 任务列表
        println!("Testing get_routine_loads...");
        match client.get_routine_loads().await {
            Ok(response) => {
                println!("✓ Successfully got routine loads");
                println!("  Total jobs: {}", response.total);
                println!("  Jobs:");
                for job in &response.jobs {
                    println!("    - Name: {}, Database: {}, Table: {}, State: {}",
                        job.name, job.database, job.table, job.state);
                }

                // 验证响应结构
                assert!(response.total == response.jobs.len(),
                    "Total count should match actual jobs length");
            }
            Err(e) => {
                eprintln!("✗ Failed to get routine loads: {}", e);
                panic!("Test failed: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_doris_get_databases() {
        dotenv::dotenv().ok();

        let host = std::env::var("DORIS_HOST")
            .expect("DORIS_HOST must be set");
        let port = std::env::var("DORIS_PORT")
            .unwrap_or_else(|_| "9030".to_string());
        let username = std::env::var("DORIS_USERNAME")
            .expect("DORIS_USERNAME must be set");
        let password = std::env::var("DORIS_PASSWORD")
            .expect("DORIS_PASSWORD must be set");
        let database = std::env::var("DORIS_DB")
            .expect("DORIS_DB must be set");
        let http_url = std::env::var("DORIS_HTTP_URL").ok();

        println!("Testing get_databases...");
        let client = DorisClient::new(host, port, username, password, database, http_url);

        match client.get_databases().await {
            Ok(response) => {
                println!("✓ Successfully got databases");
                println!("  Total databases: {}", response.count);
                println!("  Databases: {:?}", response.databases);

                assert!(response.count == response.databases.len(),
                    "Total count should match actual databases length");
                assert!(!response.databases.is_empty(),
                    "Should have at least one database");
            }
            Err(e) => {
                eprintln!("✗ Failed to get databases: {}", e);
                panic!("Test failed: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_doris_get_tables() {
        dotenv::dotenv().ok();

        let host = std::env::var("DORIS_HOST")
            .expect("DORIS_HOST must be set");
        let port = std::env::var("DORIS_PORT")
            .unwrap_or_else(|_| "9030".to_string());
        let username = std::env::var("DORIS_USERNAME")
            .expect("DORIS_USERNAME must be set");
        let password = std::env::var("DORIS_PASSWORD")
            .expect("DORIS_PASSWORD must be set");
        let database = std::env::var("DORIS_DB")
            .expect("DORIS_DB must be set");
        let http_url = std::env::var("DORIS_HTTP_URL").ok();

        println!("Testing get_tables for database: {}", database);
        let client = DorisClient::new(host, port, username, password, database.clone(), http_url);

        match client.get_tables(&database).await {
            Ok(response) => {
                println!("✓ Successfully got tables");
                println!("  Database: {}", response.database);
                println!("  Total tables: {}", response.count);
                if !response.tables.is_empty() {
                    println!("  Tables (first 10): {:?}", response.tables.iter().take(10).collect::<Vec<_>>());
                }

                assert_eq!(response.database, database,
                    "Database name should match");
                assert!(response.count == response.tables.len(),
                    "Total count should match actual tables length");
            }
            Err(e) => {
                eprintln!("✗ Failed to get tables: {}", e);
                panic!("Test failed: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_doris_get_fe_status() {
        dotenv::dotenv().ok();

        let host = std::env::var("DORIS_HOST")
            .expect("DORIS_HOST must be set");
        let port = std::env::var("DORIS_PORT")
            .unwrap_or_else(|_| "9030".to_string());
        let username = std::env::var("DORIS_USERNAME")
            .expect("DORIS_USERNAME must be set");
        let password = std::env::var("DORIS_PASSWORD")
            .expect("DORIS_PASSWORD must be set");
        let database = std::env::var("DORIS_DB")
            .expect("DORIS_DB must be set");

        // 这个测试需要 HTTP URL
        let http_url = match std::env::var("DORIS_HTTP_URL") {
            Ok(url) => url,
            Err(_) => {
                println!("⊘ Skipping test_doris_get_fe_status: DORIS_HTTP_URL not set");
                return;
            }
        };

        println!("Testing get_fe_status...");
        let client = DorisClient::new(host, port, username, password, database, Some(http_url));

        match client.get_fe_status().await {
            Ok(status) => {
                println!("✓ Successfully got FE status");
                println!("  Status: {}", status.status);
                if let Some(name) = &status.name {
                    println!("  Name: {}", name);
                }

                assert!(!status.status.is_empty(), "Status should not be empty");
            }
            Err(e) => {
                eprintln!("✗ Failed to get FE status: {}", e);
                panic!("Test failed: {}", e);
            }
        }
    }
}
