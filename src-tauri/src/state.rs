use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

use crate::config::AppConfig;
use crate::devices::DeviceStateCache;
use crate::services::ha_client::HomeAssistantService;
use crate::services::llm::{ChatMessage, LlmService, ToolExecutor};

pub struct AppState {
    pub conversation: Mutex<Vec<ChatMessage>>,
    pub llm: RwLock<Box<dyn LlmService>>,
    pub ha: RwLock<Arc<dyn HomeAssistantService>>,
    pub device_cache: Arc<DeviceStateCache>,
    pub tool_executor: RwLock<Arc<dyn ToolExecutor>>,
    pub config: RwLock<AppConfig>,
}
