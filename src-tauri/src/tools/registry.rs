use super::ToolDefinition;
use super::{climate, light, media_player, switch};

pub fn get_all_tools() -> Vec<ToolDefinition> {
    let mut tools = Vec::new();
    tools.extend(light::tools());
    tools.extend(switch::tools());
    tools.extend(climate::tools());
    tools.extend(media_player::tools());
    tools
}

pub fn find_tool(name: &str) -> Option<ToolDefinition> {
    get_all_tools().into_iter().find(|t| t.name == name)
}

/// Convert tool definitions into the format Ollama expects in the `tools` field
/// of its /api/chat request.
pub fn tools_for_ollama() -> Vec<serde_json::Value> {
    get_all_tools()
        .into_iter()
        .map(|tool| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": {
                        "type": tool.parameters.r#type,
                        "properties": tool.parameters.properties,
                        "required": tool.parameters.required,
                    }
                }
            })
        })
        .collect()
}
