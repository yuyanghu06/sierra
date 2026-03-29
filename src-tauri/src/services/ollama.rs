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
            client: Client::builder()
                .connect_timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| Client::new()),
            base_url,
            model,
        }
    }

    /// Parse a single NDJSON line from the Ollama stream.
    fn parse_chunk(line: &str) -> Result<OllamaChatResponse, String> {
        serde_json::from_str(line).map_err(|e| format!("JSON parse error: {}", e))
    }

    /// Stream a response from Ollama, parsing NDJSON line by line.
    /// Sends text tokens via `on_token`. Returns the final response (which
    /// may contain tool_calls on its `done` chunk).
    async fn consume_stream<F>(
        &self,
        response: reqwest::Response,
        mut on_token: F,
    ) -> Result<OllamaChatResponse, String>
    where
        F: FnMut(&str, bool),
    {
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

                let parsed = Self::parse_chunk(line)?;

                if !parsed.message.content.is_empty() {
                    on_token(&parsed.message.content, parsed.done);
                }

                last_response = Some(parsed);
            }
        }

        // Handle remaining buffer
        let remaining = buffer.trim();
        if !remaining.is_empty() {
            if let Ok(parsed) = serde_json::from_str::<OllamaChatResponse>(remaining) {
                if !parsed.message.content.is_empty() {
                    on_token(&parsed.message.content, parsed.done);
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
        let tool_count = tools.map(|t| t.len()).unwrap_or(0);
        println!(
            "[ollama] POST /api/chat model={} messages={} stream={} tools={}",
            self.model,
            messages.len(),
            stream,
            tool_count,
        );

        let start = std::time::Instant::now();
        let response = self
            .send_request(messages, stream, tools)
            .send()
            .await
            .map_err(|e| {
                println!("[ollama] POST /api/chat FAILED after {:?}: {}", start.elapsed(), e);
                format!("Failed to connect to Ollama: {}", e)
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            println!("[ollama] POST /api/chat returned {} after {:?}: {}", status, start.elapsed(), body);
            return Err(format!("Ollama returned {}: {}", status, body));
        }

        println!("[ollama] POST /api/chat connected {} after {:?}", status, start.elapsed());
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
        println!("[ollama] chat_stream starting");
        let start = std::time::Instant::now();
        let response = self.post_chat(&messages, true, None).await?;
        self.consume_stream(response, |content, done| {
            let _ = tx.try_send(StreamChunk {
                content: content.to_string(),
                done,
            });
        })
        .await?;
        println!("[ollama] chat_stream completed in {:?}", start.elapsed());
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
        let overall_start = std::time::Instant::now();
        println!("[ollama] chat_with_tools starting ({} tools registered)", tools.len());

        for round in 0..max_tool_rounds {
            println!("[ollama] chat_with_tools round {} — {} messages in conversation", round + 1, conversation.len());
            let round_start = std::time::Instant::now();

            let response = self
                .post_chat(&conversation, true, Some(&tools))
                .await?;

            let tx_ref = &tx;
            let mut streamed_content = String::new();

            let final_resp = self
                .consume_stream(response, |content, done| {
                    streamed_content.push_str(content);
                    let _ = tx_ref.try_send(LlmEvent::Token(StreamChunk {
                        content: content.to_string(),
                        done,
                    }));
                })
                .await?;

            println!("[ollama] chat_with_tools round {} stream completed in {:?} ({} chars)", round + 1, round_start.elapsed(), streamed_content.len());

            let has_tool_calls = final_resp
                .message
                .tool_calls
                .as_ref()
                .map(|tc| !tc.is_empty())
                .unwrap_or(false);

            if !has_tool_calls {
                println!("[ollama] chat_with_tools finished (text response) in {:?}", overall_start.elapsed());
                conversation.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: streamed_content,
                    tool_calls: None,
                });

                return Ok(conversation);
            }

            // Process tool calls
            let tool_calls = final_resp.message.tool_calls.unwrap();
            println!(
                "[ollama] chat_with_tools round {} — {} tool call(s): [{}]",
                round + 1,
                tool_calls.len(),
                tool_calls.iter().map(|tc| tc.function.name.as_str()).collect::<Vec<_>>().join(", ")
            );

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
                content: streamed_content,
                tool_calls: Some(tc_for_message),
            });

            for tc in &tool_calls {
                let tool_name = &tc.function.name;
                let arguments = &tc.function.arguments;

                println!("[ollama] executing tool {} with args: {}", tool_name, arguments);
                let exec_start = std::time::Instant::now();

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

                println!(
                    "[ollama] tool {} {} in {:?}: {}",
                    tool_name,
                    if success { "succeeded" } else { "failed" },
                    exec_start.elapsed(),
                    result_content
                );

                let _ = tx
                    .send(LlmEvent::ToolCallCompleted(ToolCallEvent {
                        tool_name: tool_name.clone(),
                        arguments: arguments.clone(),
                        success,
                        result_message: result_content.clone(),
                    }))
                    .await;

                conversation.push(ChatMessage {
                    role: "tool".to_string(),
                    content: result_content,
                    tool_calls: None,
                });
            }
        }

        println!("[ollama] chat_with_tools exceeded max rounds after {:?}", overall_start.elapsed());
        Err("Tool calling loop exceeded maximum rounds".to_string())
    }

    async fn is_healthy(&self) -> bool {
        println!("[ollama] health check GET {}", self.base_url);
        let result = self
            .client
            .get(&self.base_url)
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        println!("[ollama] health check: {}", if result { "healthy" } else { "unreachable" });
        result
    }

    async fn list_models(&self) -> Result<Vec<String>, String> {
        println!("[ollama] GET /api/tags");
        let response = self
            .client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await
            .map_err(|e| {
                println!("[ollama] GET /api/tags FAILED: {}", e);
                format!("Failed to list models: {}", e)
            })?;

        let tags: OllamaTagsResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse model list: {}", e))?;

        let names: Vec<String> = tags.models.into_iter().map(|m| m.name).collect();
        println!("[ollama] listed {} models", names.len());
        Ok(names)
    }
}
