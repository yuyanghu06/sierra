mod commands;
mod config;
mod devices;
mod prompts;
mod services;
mod state;
mod tools;

use services::ha_client::HaRestClient;
use services::mcp_server::{self, McpServerState};
use services::ollama::OllamaService;
use services::tool_executor::HaToolExecutor;
use state::AppState;
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tokio::sync::RwLock;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Load persisted config
            let cfg = config::load_config(app.handle());

            let ollama_url = cfg
                .ollama_url
                .clone()
                .unwrap_or_else(|| "http://localhost:11434".to_string());
            let ollama_model = cfg
                .ollama_model
                .clone()
                .unwrap_or_else(|| "qwen3.5:4b".to_string());
            let ha_url = cfg
                .ha_url
                .clone()
                .unwrap_or_else(|| "http://localhost:8123".to_string());
            let ha_token = cfg.ha_token.clone().unwrap_or_default();

            let ollama = OllamaService::new(ollama_url, ollama_model);

            let ha_client: Arc<dyn services::ha_client::HomeAssistantService> =
                Arc::new(HaRestClient::new(ha_url, ha_token));

            let device_cache = devices::new_shared_cache();
            let tool_executor: Arc<dyn services::llm::ToolExecutor> =
                Arc::new(HaToolExecutor::new(ha_client.clone()));

            let mcp_state = Arc::new(McpServerState {
                ha_client: ha_client.clone(),
            });

            app.manage(AppState {
                conversation: Mutex::new(Vec::new()),
                llm: RwLock::new(Box::new(ollama)),
                ha: RwLock::new(ha_client.clone()),
                device_cache: device_cache.clone(),
                tool_executor: RwLock::new(tool_executor),
                config: RwLock::new(cfg),
            });

            let ha_client_setup = ha_client.clone();
            let device_cache_setup = device_cache.clone();

            // Populate device cache from HA on startup
            tauri::async_runtime::spawn(async move {
                if ha_client_setup.is_healthy().await {
                    if let Ok(states) = ha_client_setup.get_all_states().await {
                        device_cache_setup.populate(states).await;
                    }
                }
            });

            // Start MCP server on port 3001
            tauri::async_runtime::spawn(async move {
                if let Err(e) = mcp_server::start_mcp_server(mcp_state, 3001).await {
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
            commands::settings::get_config,
            commands::settings::save_config,
            commands::settings::test_ha_connection,
            commands::settings::get_active_model,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
