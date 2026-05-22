use crate::config::db::Database;
use anyhow::{Context, Result};
use std::fs;

///
/// 主要是用来加载配置信息的，读取到配置文件之后
/// 我们需要根据具体的配置信息生成对应的Tool数据结构体
///
pub mod args;
pub mod db;
pub mod tool;

pub fn load_config(config_path: &str) -> Result<tool::ToolsConfig> {
    let content = fs::read_to_string(config_path)
        .with_context(|| format!("read config file failed: {config_path}"))?;
    serde_yaml::from_str(&content)
        .with_context(|| format!("parse config file failed: {config_path}"))
}

pub fn load_db_config() -> db::Database {
    Database::load_from_env()
}
