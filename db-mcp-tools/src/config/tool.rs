use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

type McpJsonObject = rmcp::model::JsonObject;
type McpTool = rmcp::model::Tool;

#[derive(Serialize, Deserialize)]
pub struct ToolsConfig {
    tools: Vec<Tool>,
}

impl ToolsConfig {
    pub fn mcp_tools(&self) -> Vec<McpTool> {
        self.tools.iter().map(Tool::to_mcp_tool).collect()
    }
}

#[derive(Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "name")]
    name: String,
    description: String,
    properties: Properties,
    required: Vec<String>,
}

impl Tool {
    fn to_mcp_tool(&self) -> McpTool {
        McpTool::new(
            self.name.clone(),
            self.description.clone(),
            self.properties.to_mcp_json_object(),
        )
    }
}

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct Properties(pub HashMap<String, FieldProperty>);

impl Properties {
    fn to_mcp_json_object(&self) -> Arc<McpJsonObject> {
        let json = rmcp::model::object(
            serde_json::to_value(self).expect("serialize to serde json value failed"),
        );
        Arc::new(json)
    }
}

#[derive(Serialize, Deserialize)]
pub struct FieldProperty {
    #[serde(rename = "type")]
    prop_type: String,
    description: String,
}
