use anyhow::{Result, anyhow, bail};
use serde_json::{Map, Number, Value};
use sqlx::{Column, MySqlPool, Row, TypeInfo};

use crate::config::load_db_config;

pub struct DatabaseExecutor {
    pool: MySqlPool,
}

impl DatabaseExecutor {
    pub async fn new() -> Self {
        let db_config = load_db_config();
        let url = db_config.url();
        let pool = MySqlPool::connect(url.as_str())
            .await
            .expect("Connection to mysql error");

        Self { pool }
    }

    pub async fn execute(&self, query: &str, params: &Map<String, Value>) -> Result<Value> {
        let sql = Self::render_sql(query, params)?;
        let rows = sqlx::query(sql.as_str()).fetch_all(&self.pool).await?;
        let mut values = Vec::with_capacity(rows.len());

        for row in rows {
            let mut object = Map::new();

            for column in row.columns() {
                let name = column.name();
                let value = Self::mysql_column_to_json(&row, name, column.type_info().name());
                object.insert(name.to_string(), value);
            }

            values.push(Value::Object(object));
        }

        Ok(Value::Array(values))
    }

    pub fn render_sql(query: &str, params: &Map<String, Value>) -> Result<String> {
        let mut rendered = String::with_capacity(query.len());
        let mut rest = query;

        while let Some(start) = rest.find("#{") {
            rendered.push_str(&rest[..start]);
            let placeholder = &rest[start + 2..];
            let Some(end) = placeholder.find('}') else {
                bail!("SQL placeholder missing closing brace: {}", rest);
            };

            let name = &placeholder[..end];
            let value = params
                .get(name)
                .ok_or_else(|| anyhow!("missing SQL parameter: {}", name))?;

            rendered.push_str(&Self::value_to_sql_text(value));
            rest = &placeholder[end + 1..];
        }

        rendered.push_str(rest);
        Ok(rendered)
    }

    fn mysql_column_to_json(row: &sqlx::mysql::MySqlRow, name: &str, type_name: &str) -> Value {
        match type_name {
            "TINYINT" | "SMALLINT" | "INT" | "MEDIUMINT" | "BIGINT" => row
                .try_get::<Option<i64>, _>(name)
                .ok()
                .flatten()
                .map(|value| Value::Number(Number::from(value)))
                .unwrap_or(Value::Null),
            "FLOAT" | "DOUBLE" | "DECIMAL" => row
                .try_get::<Option<f64>, _>(name)
                .ok()
                .flatten()
                .and_then(Number::from_f64)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            _ => row
                .try_get::<Option<String>, _>(name)
                .ok()
                .flatten()
                .map(Value::String)
                .unwrap_or(Value::Null),
        }
    }

    fn value_to_sql_text(value: &Value) -> String {
        match value {
            Value::String(value) => value.clone(),
            Value::Number(value) => value.to_string(),
            Value::Bool(value) => value.to_string(),
            Value::Null => "null".to_string(),
            other => other.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::DatabaseExecutor;

    #[test]
    fn render_sql_replaces_mybatis_style_placeholder() {
        let params = json!({
            "name": "liuxu"
        })
        .as_object()
        .unwrap()
        .clone();

        let sql = DatabaseExecutor::render_sql("where user_name = #{name}", &params).unwrap();

        assert_eq!(sql, "where user_name = liuxu");
    }

    #[test]
    fn render_sql_replaces_multiple_placeholders() {
        let params = json!({
            "name": "liuxu",
            "limit": 10
        })
        .as_object()
        .unwrap()
        .clone();

        let sql = DatabaseExecutor::render_sql("where user_name = #{name} limit #{limit}", &params)
            .unwrap();

        assert_eq!(sql, "where user_name = liuxu limit 10");
    }

    #[test]
    fn render_sql_returns_error_when_param_missing() {
        let params = json!({}).as_object().unwrap().clone();

        let error = DatabaseExecutor::render_sql("where user_name = #{name}", &params).unwrap_err();

        assert_eq!(error.to_string(), "missing SQL parameter: name");
    }
}
