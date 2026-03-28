use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamChunk {
    pub content: String,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolCallEvent {
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub success: bool,
    pub result_message: String,
}

/// Events sent from the LLM service to the command layer during a chat interaction.
#[derive(Debug, Clone, Serialize)]
pub enum LlmEvent {
    Token(StreamChunk),
    ToolCallStarted { tool_name: String },
    ToolCallCompleted(ToolCallEvent),
}

#[async_trait::async_trait]
pub trait LlmService: Send + Sync {
    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        tx: mpsc::Sender<StreamChunk>,
    ) -> Result<(), String>;

    /// Chat with tool calling support. Sends LlmEvents that include both tokens
    /// and tool call status. Returns the final conversation messages (including
    /// any tool call/result messages that were added during the loop).
    async fn chat_with_tools(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<serde_json::Value>,
        tool_executor: &dyn ToolExecutor,
        tx: mpsc::Sender<LlmEvent>,
    ) -> Result<Vec<ChatMessage>, String>;

    async fn is_healthy(&self) -> bool;

    async fn list_models(&self) -> Result<Vec<String>, String>;
}

/// Executes a tool call and returns the result as a string.
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(
        &self,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<String, String>;
}
