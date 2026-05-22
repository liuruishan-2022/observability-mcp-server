use crate::Args;
use std::fs;
use tracing::info;

///
/// 主要是用来加载配置信息的，读取到配置文件之后
/// 我们需要根据具体的配置信息生成对应的Tool数据结构体
///
pub mod args;
pub mod db;
pub mod tool;

pub fn load_config() -> tool::ToolsConfig {
    let args = Args::parse_args();
    info!("args:{}", args.to_string());
    let config_path = args.config().to_string();
    let content = fs::read_to_string(config_path).expect("read config file error");
    serde_yaml::from_str(&content).expect("Unable to parse config file content")
}
