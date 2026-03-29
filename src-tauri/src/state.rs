use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

use crate::config::AppConfig;
use crate::devices::DeviceStateCache;
use crate::services::ha_client::HomeAssistantService;
use crate::services::llm::{ChatMessage, LlmService, ToolExecutor};

pub struct AppState {
    pub conversation: Mutex<Vec<ChatMessage>>,
    pub llm: RwLock<Box<dyn LlmService>>,
    /// Wrapped in Arc so it can be shared with McpServerState and updated in-place
    /// when the effective HA URL is determined after startup.
    pub ha: Arc<RwLock<Arc<dyn HomeAssistantService>>>,
    pub device_cache: Arc<DeviceStateCache>,
    pub tool_executor: RwLock<Arc<dyn ToolExecutor>>,
    pub config: RwLock<AppConfig>,
}
