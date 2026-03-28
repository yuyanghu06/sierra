mod commands;
mod devices;
mod prompts;
mod services;
mod state;
mod tools;

use services::ha_client::HaRestClient;
use services::ha_ws::HaWebSocketClient;
use services::mcp_server::{self, McpServerState};
use services::ollama::OllamaService;
use services::tool_executor::HaToolExecutor;
use state::AppState;
use std::sync::{Arc, Mutex};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let ollama = OllamaService::new(
        "http://localhost:11434".to_string(),
        "qwen3.5:4b".to_string(),
    );

    // HA client — defaults to localhost; reconfigured when user enters credentials in Settings
    let ha_client: Arc<dyn services::ha_client::HomeAssistantService> = Arc::new(
        HaRestClient::new(
            "http://localhost:8123".to_string(),
            String::new(),
        ),
    );

    let device_cache = devices::new_shared_cache();
    let tool_executor: Arc<dyn services::llm::ToolExecutor> =
        Arc::new(HaToolExecutor::new(ha_client.clone()));

    // MCP server state
    let mcp_state = Arc::new(McpServerState {
        ha_client: ha_client.clone(),
    });

    tauri::Builder::default()
        .manage(AppState {
            conversation: Mutex::new(Vec::new()),
            llm: Box::new(ollama),
            ha: ha_client.clone(),
            device_cache: device_cache.clone(),
            tool_executor,
        })
        .setup(move |_app| {
            let ha_client_setup = ha_client.clone();
            let device_cache_setup = device_cache.clone();
            let mcp_state_setup = mcp_state.clone();

            // Spawn background tasks
            tauri::async_runtime::spawn(async move {
                // Try to populate device cache from HA on startup
                if ha_client_setup.is_healthy().await {
                    if let Ok(states) = ha_client_setup.get_all_states().await {
                        device_cache_setup.populate(states).await;
                    }
                }
            });

            // Start MCP server on port 3001
            tauri::async_runtime::spawn(async move {
                if let Err(e) = mcp_server::start_mcp_server(mcp_state_setup, 3001).await {
                    eprintln!("MCP server error: {}", e);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::ping,
            commands::system::get_app_info,
            commands::chat::send_chat_message,
            commands::chat::clear_conversation,
            commands::chat::check_ollama_health,
            commands::chat::list_models,
            commands::devices::get_all_devices,
            commands::devices::get_device_state,
            commands::devices::get_rooms,
            commands::devices::get_device_count,
            commands::devices::call_device_action,
            commands::devices::check_ha_health,
            commands::devices::refresh_devices,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
