use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::llm::{ChatMessage, LlmService, StreamChunk};

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
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: OllamaMessageResponse,
    done: bool,
}

#[derive(Deserialize)]
struct OllamaMessageResponse {
    content: String,
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
}

#[async_trait::async_trait]
impl LlmService for OllamaService {
    async fn chat_stream(
        &self,
        messages: Vec<ChatMessage>,
        tx: mpsc::Sender<StreamChunk>,
    ) -> Result<(), String> {
        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages,
            stream: true,
        };

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Failed to connect to Ollama: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Ollama returned {}: {}", status, body));
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(item) = stream.next().await {
            let bytes = item.map_err(|e| format!("Stream error: {}", e))?;
            let text = std::str::from_utf8(&bytes).map_err(|e| format!("UTF-8 error: {}", e))?;
            buffer.push_str(text);

            // Process complete lines from the buffer
            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                let parsed: OllamaChatResponse =
                    serde_json::from_str(line).map_err(|e| format!("JSON parse error: {}", e))?;

                tx.send(StreamChunk {
                    content: parsed.message.content,
                    done: parsed.done,
                })
                .await
                .map_err(|e| format!("Channel send error: {}", e))?;
            }
        }

        // Handle any remaining data in the buffer
        let remaining = buffer.trim();
        if !remaining.is_empty() {
            if let Ok(parsed) = serde_json::from_str::<OllamaChatResponse>(remaining) {
                let _ = tx
                    .send(StreamChunk {
                        content: parsed.message.content,
                        done: parsed.done,
                    })
                    .await;
            }
        }

        Ok(())
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
