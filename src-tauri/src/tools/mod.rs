pub mod climate;
pub mod light;
pub mod media_player;
pub mod registry;
pub mod switch;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub domain: String,
    pub service: String,
    pub parameters: ToolParameters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameters {
    pub r#type: String,
    pub properties: serde_json::Value,
    pub required: Vec<String>,
}
