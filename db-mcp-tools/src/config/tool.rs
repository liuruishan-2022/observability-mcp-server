use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

type McpJsonObject = rmcp::model::JsonObject;
type McpTool = rmcp::model::Tool;
type Properties = HashMap<String, FieldProperty>;

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
        let input_schema = InputSchema::new(&self.properties);
        McpTool::new(
            self.name.clone(),
            self.description.clone(),
            input_schema.to_mcp_json_object(),
        )
    }
}

#[derive(Serialize)]
pub struct InputSchema<'a> {
    #[serde(rename = "type")]
    schema_type: String,
    properties: &'a Properties,
}

impl<'a> InputSchema<'a> {
    pub fn new(properties: &'a Properties) -> Self {
        InputSchema {
            schema_type: "object".to_string(),
            properties,
        }
    }

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
