use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

type McpJsonObject = rmcp::model::JsonObject;
type McpTool = rmcp::model::Tool;
type Properties = HashMap<String, FieldProperty>;

#[derive(Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    tools: Vec<Tool>,
}

impl ToolsConfig {
    pub fn mcp_tools(&self) -> Vec<ToolWrapper> {
        self.tools
            .iter()
            .map(|ele| ToolWrapper {
                tool: ele.to_mcp_tool(),
                config: ele.clone(),
            })
            .collect()
    }

    pub fn tools(&self) -> &[Tool] {
        &self.tools
    }

    pub fn find_tool(&self, name: &str) -> Option<&Tool> {
        self.tools.iter().find(|tool| tool.name() == name)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "name")]
    name: String,
    description: String,
    sql: String,
    properties: Properties,
    required: Vec<String>,
}

impl Tool {
    pub fn to_mcp_tool(&self) -> McpTool {
        let input_schema = InputSchema::new(&self.properties, &self.required);
        McpTool::new(
            self.name.clone(),
            self.description.clone(),
            input_schema.to_mcp_json_object(),
        )
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn sql(&self) -> &str {
        self.sql.as_str()
    }
}

#[derive(Serialize)]
pub struct InputSchema<'a> {
    #[serde(rename = "type")]
    schema_type: String,
    properties: &'a Properties,
    required: &'a [String],
}

impl<'a> InputSchema<'a> {
    pub fn new(properties: &'a Properties, required: &'a [String]) -> Self {
        InputSchema {
            schema_type: "object".to_string(),
            properties,
            required,
        }
    }

    fn to_mcp_json_object(&self) -> Arc<McpJsonObject> {
        let json = rmcp::model::object(
            serde_json::to_value(self).expect("serialize to serde json value failed"),
        );
        Arc::new(json)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FieldProperty {
    #[serde(rename = "type")]
    prop_type: String,
    description: String,
}

pub struct ToolWrapper {
    tool: McpTool,
    config: Tool,
}

impl ToolWrapper {
    pub fn into_parts(self) -> (McpTool, Tool) {
        (self.tool, self.config)
    }
}
