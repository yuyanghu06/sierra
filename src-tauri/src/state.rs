use std::sync::Mutex;

use crate::services::llm::{ChatMessage, LlmService};

pub struct AppState {
    pub conversation: Mutex<Vec<ChatMessage>>,
    pub llm: Box<dyn LlmService>,
}
