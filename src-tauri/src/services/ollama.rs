use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::llm::{
    ChatMessage, LlmEvent, LlmService, StreamChunk, ToolCall, ToolCallEvent, ToolCallFunction,
    ToolExecutor,
};

pub struct OllamaService {
    client: Client,
    base_url: String,
    model: String,
}

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessageResponse,
    done: bool,
}

#[derive(Deserialize)]
struct OllamaMessageResponse {
    #[serde(default)]
    content: String,
    #[serde(default)]
    #[allow(dead_code)]
    role: String,
    #[serde(default)]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Deserialize)]
struct OllamaToolCall {
    function: OllamaToolCallFunction,
}

#[derive(Deserialize)]
struct OllamaToolCallFunction {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelEntry>,
}

#[derive(Deserialize)]
struct OllamaModelEntry {
    name: String,
}

impl OllamaService {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
            model,
        }
    }

    /// Stream a response from Ollama, parsing NDJSON line by line.
    /// Returns the collected full response once done.
    async fn stream_response(
        &self,
        response: reqwest::Response,
        tx: &mpsc::Sender<StreamChunk>,
    ) -> Result<OllamaChatResponse, String> {
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut last_response: Option<OllamaChatResponse> = None;

        while let Some(item) = stream.next().await {
            let bytes = item.map_err(|e| format!("Stream error: {}", e))?;
            let text = std::str::from_utf8(&bytes).map_err(|e| format!("UTF-8 error: {}", e))?;
            buffer.push_str(text);

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                let parsed: OllamaChatResponse =
                    serde_json::from_str(line).map_err(|e| format!("JSON parse error: {}", e))?;

                if !parsed.message.content.is_empty() {
                    let _ = tx
                        .send(StreamChunk {
                            content: parsed.message.content.clone(),
                            done: parsed.done,
                        })
                        .await;
                }

                if parsed.done {
                    last_response = Some(parsed);
                } else {
                    last_response = Some(parsed);
                }
            }
        }

        // Handle remaining buffer
        let remaining = buffer.trim();
        if !remaining.is_empty() {
            if let Ok(parsed) = serde_json::from_str::<OllamaChatResponse>(remaining) {
                if !parsed.message.content.is_empty() {
                    let _ = tx
                        .send(StreamChunk {
                            content: parsed.message.content.clone(),
                            done: parsed.done,
                        })
                        .await;
                }
                last_response = Some(parsed);
            }
        }

        last_response.ok_or_else(|| "No response received from Ollama".to_string())
    }

    fn send_request(
        &self,
        messages: &[ChatMessage],
        stream: bool,
        tools: Option<&[serde_json::Value]>,
    ) -> reqwest::RequestBuilder {
        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            stream,
            tools: tools.map(|t| t.to_vec()),
        };

        self.client
            .post(format!("{}/api/chat", self.base_url))
            .json(&request)
    }

    async fn post_chat(
        &self,
        messages: &[ChatMessage],
        stream: bool,
        tools: Option<&[serde_json::Value]>,
    ) -> Result<reqwest::Response, String> {
        let response = self
            .send_request(messages, stream, tools)
            .send()
            .await
            .map_err(|e| format!("Failed to connect to Ollama: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Ollama returned {}: {}", status, body));
        }

        Ok(response)
    }
}

#[async_trait::async_trait]
impl LlmService for OllamaService {
    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        tx: mpsc::Sender<StreamChunk>,
    ) -> Result<(), String> {
        let response = self.post_chat(&messages, true, None).await?;
        self.stream_response(response, &tx).await?;
        Ok(())
    }

    async fn chat_with_tools(
        &self,
        messages: Vec<ChatMessage>,
        tools: Vec<serde_json::Value>,
        tool_executor: &dyn ToolExecutor,
        tx: mpsc::Sender<LlmEvent>,
    ) -> Result<Vec<ChatMessage>, String> {
        let mut conversation = messages;
        let max_tool_rounds = 10;

        for _ in 0..max_tool_rounds {
            // Non-streaming request when tools are available, so we can inspect tool_calls
            let response = self
                .post_chat(&conversation, false, Some(&tools))
                .await?;

            let body: OllamaChatResponse = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

            let has_tool_calls = body
                .message
                .tool_calls
                .as_ref()
                .map(|tc| !tc.is_empty())
                .unwrap_or(false);

            if !has_tool_calls {
                // Final text response — send the content as tokens
                if !body.message.content.is_empty() {
                    let _ = tx
                        .send(LlmEvent::Token(StreamChunk {
                            content: body.message.content.clone(),
                            done: true,
                        }))
                        .await;
                }

                conversation.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: body.message.content,
                    tool_calls: None,
                });

                return Ok(conversation);
            }

            // Process tool calls
            let tool_calls = body.message.tool_calls.unwrap();

            // Add the assistant message with tool_calls to conversation
            let tc_for_message: Vec<ToolCall> = tool_calls
                .iter()
                .map(|tc| ToolCall {
                    function: ToolCallFunction {
                        name: tc.function.name.clone(),
                        arguments: tc.function.arguments.clone(),
                    },
                })
                .collect();

            conversation.push(ChatMessage {
                role: "assistant".to_string(),
                content: body.message.content.clone(),
                tool_calls: Some(tc_for_message),
            });

            // Execute each tool call
            for tc in &tool_calls {
                let tool_name = &tc.function.name;
                let arguments = &tc.function.arguments;

                let _ = tx
                    .send(LlmEvent::ToolCallStarted {
                        tool_name: tool_name.clone(),
                    })
                    .await;

                let (success, result_content) =
                    match tool_executor.execute(tool_name, arguments).await {
                        Ok(result) => (true, result),
                        Err(e) => (false, format!("Error: {}", e)),
                    };

                let _ = tx
                    .send(LlmEvent::ToolCallCompleted(ToolCallEvent {
                        tool_name: tool_name.clone(),
                        arguments: arguments.clone(),
                        success,
                        result_message: result_content.clone(),
                    }))
                    .await;

                // Add tool result to conversation
                conversation.push(ChatMessage {
                    role: "tool".to_string(),
                    content: result_content,
                    tool_calls: None,
                });
            }
        }

        Err("Tool calling loop exceeded maximum rounds".to_string())
    }

    async fn is_healthy(&self) -> bool {
        self.client
            .get(&self.base_url)
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    async fn list_models(&self) -> Result<Vec<String>, String> {
        let response = self
            .client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .map_err(|e| format!("Failed to list models: {}", e))?;

        let tags: OllamaTagsResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse model list: {}", e))?;

        Ok(tags.models.into_iter().map(|m| m.name).collect())
    }
}
