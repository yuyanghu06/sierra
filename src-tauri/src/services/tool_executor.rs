use std::sync::Arc;

use crate::services::ha_client::HomeAssistantService;
use crate::services::llm::ToolExecutor;
use crate::tools::registry;

pub struct HaToolExecutor {
    ha_client: Arc<dyn HomeAssistantService>,
}

impl HaToolExecutor {
    pub fn new(ha_client: Arc<dyn HomeAssistantService>) -> Self {
        Self { ha_client }
    }
}

#[async_trait::async_trait]
impl ToolExecutor for HaToolExecutor {
    async fn execute(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<String, String> {
        let tool = registry::find_tool(tool_name)
            .ok_or_else(|| format!("Unknown tool: {}", tool_name))?;

        let entity_id = arguments
            .get("entity_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing required parameter: entity_id".to_string())?;

        // Build extra data by removing entity_id from arguments
        let extra_data = if let serde_json::Value::Object(mut map) = arguments.clone() {
            map.remove("entity_id");
            if map.is_empty() {
                None
            } else {
                Some(serde_json::Value::Object(map))
            }
        } else {
            None
        };

        self.ha_client
            .call_service(&tool.domain, &tool.service, entity_id, extra_data)
            .await?;

        Ok(format!(
            "Successfully executed {}.{} on {}",
            tool.domain, tool.service, entity_id
        ))
    }
}
