use serde::Serialize;
use tauri::ipc::Channel;

use crate::services::llm::ChatMessage;
use crate::state::AppState;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "data")]
pub enum ChatEvent {
    Token(String),
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
        });
        conv.clone()
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel::<crate::services::llm::StreamChunk>(32);

    // Spawn relay task: reads from mpsc channel, forwards to Tauri Channel
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

    // Run the LLM stream (blocks until Ollama finishes, drops tx when done)
    if let Err(e) = state.llm.chat_stream(messages, tx).await {
        on_event
            .send(ChatEvent::Error(e.clone()))
            .map_err(|e| e.to_string())?;
        return Err(e);
    }

    // Wait for the relay to drain and get the full response
    let full_response = relay_handle.await.map_err(|e| e.to_string())?;

    // Add assistant response to history
    {
        let mut conv = state.conversation.lock().map_err(|e| e.to_string())?;
        conv.push(ChatMessage {
            role: "assistant".to_string(),
            content: full_response,
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
