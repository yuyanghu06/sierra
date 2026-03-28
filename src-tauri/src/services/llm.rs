use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamChunk {
    pub content: String,
    pub done: bool,
}

#[async_trait::async_trait]
pub trait LlmService: Send + Sync {
    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        tx: mpsc::Sender<StreamChunk>,
    ) -> Result<(), String>;

    async fn is_healthy(&self) -> bool;

    async fn list_models(&self) -> Result<Vec<String>, String>;
}
