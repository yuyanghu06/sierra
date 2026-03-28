use std::sync::{Arc, Mutex};

use crate::devices::DeviceStateCache;
use crate::services::ha_client::HomeAssistantService;
use crate::services::llm::{ChatMessage, LlmService, ToolExecutor};

pub struct AppState {
    pub conversation: Mutex<Vec<ChatMessage>>,
    pub llm: Box<dyn LlmService>,
    pub ha: Arc<dyn HomeAssistantService>,
    pub device_cache: Arc<DeviceStateCache>,
    pub tool_executor: Arc<dyn ToolExecutor>,
}
