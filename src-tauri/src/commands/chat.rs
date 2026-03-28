use serde::Serialize;
use tauri::ipc::Channel;

use crate::prompts;
use crate::services::llm::{ChatMessage, LlmEvent};
use crate::state::AppState;
use crate::tools::registry;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "data")]
pub enum ChatEvent {
    Token(String),
    ToolCallStart { tool_name: String },
    ToolCallResult {
        tool_name: String,
        success: bool,
        message: String,
    },
    Done { full_response: String },
    Error(String),
}

#[tauri::command]
pub async fn send_chat_message(
    state: tauri::State<'_, AppState>,
    message: String,
    on_event: Channel<ChatEvent>,
) -> Result<(), String> {
    // Add user message to history and take a snapshot
    let messages = {
        let mut conv = state.conversation.lock().map_err(|e| e.to_string())?;
        conv.push(ChatMessage {
            role: "user".to_string(),
            content: message,
            tool_calls: None,
        });
        conv.clone()
    };

    // Build system prompt with current device list and prepend to messages
    let system_prompt = prompts::build_system_prompt(&state.device_cache).await;
    let mut messages_with_system = vec![ChatMessage {
        role: "system".to_string(),
        content: system_prompt,
        tool_calls: None,
    }];
    messages_with_system.extend(messages);
    let messages = messages_with_system;

    // Check if HA is healthy — if so, use tool calling; otherwise plain chat
    let ha_available = state.ha.is_healthy().await;

    if ha_available {
        // Tool-calling path
        let tools = registry::tools_for_ollama();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<LlmEvent>(32);
        let tool_executor = state.tool_executor.clone();

        let on_event_clone = on_event.clone();
        let relay_handle = tokio::spawn(async move {
            let mut full_response = String::new();
            while let Some(event) = rx.recv().await {
                match event {
                    LlmEvent::Token(chunk) => {
                        full_response.push_str(&chunk.content);
                        if chunk.done {
                            let _ = on_event_clone.send(ChatEvent::Done {
                                full_response: full_response.clone(),
                            });
                        } else {
                            let _ = on_event_clone.send(ChatEvent::Token(chunk.content));
                        }
                    }
                    LlmEvent::ToolCallStarted { tool_name } => {
                        let _ = on_event_clone.send(ChatEvent::ToolCallStart { tool_name });
                    }
                    LlmEvent::ToolCallCompleted(tc) => {
                        let _ = on_event_clone.send(ChatEvent::ToolCallResult {
                            tool_name: tc.tool_name,
                            success: tc.success,
                            message: tc.result_message,
                        });
                    }
                }
            }
            full_response
        });

        let result = state
            .llm
            .chat_with_tools(messages, tools, tool_executor.as_ref(), tx)
            .await;

        match result {
            Ok(updated_messages) => {
                let _full_response = relay_handle.await.map_err(|e| e.to_string())?;

                // Replace conversation with the updated messages (includes tool rounds)
                let mut conv = state.conversation.lock().map_err(|e| e.to_string())?;
                *conv = updated_messages;
            }
            Err(e) => {
                let _ = on_event.send(ChatEvent::Error(e.clone()));
                return Err(e);
            }
        }
    } else {
        // Plain streaming path (no HA, no tools)
        let (tx, mut rx) =
            tokio::sync::mpsc::channel::<crate::services::llm::StreamChunk>(32);

        let on_event_clone = on_event.clone();
        let relay_handle = tokio::spawn(async move {
            let mut full_response = String::new();
            while let Some(chunk) = rx.recv().await {
                if chunk.done {
                    let _ = on_event_clone.send(ChatEvent::Done {
                        full_response: full_response.clone(),
                    });
                } else {
                    full_response.push_str(&chunk.content);
                    let _ = on_event_clone.send(ChatEvent::Token(chunk.content));
                }
            }
            full_response
        });

        if let Err(e) = state.llm.chat_stream(messages, tx).await {
            let _ = on_event.send(ChatEvent::Error(e.clone()));
            return Err(e);
        }

        let full_response = relay_handle.await.map_err(|e| e.to_string())?;

        let mut conv = state.conversation.lock().map_err(|e| e.to_string())?;
        conv.push(ChatMessage {
            role: "assistant".to_string(),
            content: full_response,
            tool_calls: None,
        });
    }

    Ok(())
}

#[tauri::command]
pub fn clear_conversation(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut conv = state.conversation.lock().map_err(|e| e.to_string())?;
    conv.clear();
    Ok(())
}

#[tauri::command]
pub async fn check_ollama_health(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    Ok(state.llm.is_healthy().await)
}

#[tauri::command]
pub async fn list_models(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    state.llm.list_models().await
}
