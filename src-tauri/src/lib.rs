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
use services::process_manager::ProcessManager;
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
                Arc::new(HaRestClient::new(ha_url.clone(), ha_token.clone()));

            let device_cache = devices::new_shared_cache();
            let tool_executor: Arc<dyn services::llm::ToolExecutor> =
                Arc::new(HaToolExecutor::new(ha_client.clone()));

            let mcp_state = Arc::new(McpServerState {
                ha_client: ha_client.clone(),
            });

            // Create the process manager
            let app_data_dir = app
                .handle()
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));

            let log_dir = {
                #[cfg(target_os = "macos")]
                {
                    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                    std::path::PathBuf::from(home).join("Library/Logs/Sierra")
                }
                #[cfg(target_os = "windows")]
                {
                    let appdata =
                        std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
                    std::path::PathBuf::from(appdata).join("Sierra\\logs")
                }
                #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                {
                    app_data_dir.join("logs")
                }
            };

            let process_manager = Arc::new(ProcessManager::new(app_data_dir, log_dir));

            // Manage both AppState and ProcessManager as separate Tauri states
            app.manage(AppState {
                conversation: Mutex::new(Vec::new()),
                llm: RwLock::new(Box::new(ollama)),
                ha: RwLock::new(ha_client.clone()),
                device_cache: device_cache.clone(),
                tool_executor: RwLock::new(tool_executor),
                config: RwLock::new(cfg),
            });

            app.manage(process_manager.clone());

            // Start Ollama and Home Assistant on launch
            let pm_startup = process_manager.clone();
            let ha_client_setup = ha_client.clone();
            let device_cache_setup = device_cache.clone();

            tauri::async_runtime::spawn(async move {
                // Detect external Ollama — reuse it if running, otherwise start managed.
                // For HA we always start the managed instance (token must match Sierra's config),
                // so kill any stale HA on :8123 first.
                let ollama_external = pm_startup.detect_external_ollama().await;

                if ollama_external {
                    println!("[Ollama] detected external instance — skipping managed start");
                    let mut svc = pm_startup.ollama.write().await;
                    svc.is_external = true;
                    svc.status = services::process_manager::ServiceStatus::External;
                } else if let Err(e) = pm_startup.start_ollama().await {
                    eprintln!("Failed to start Ollama: {}", e);
                }

                // Kill any HA already on :8123 so Sierra's managed instance can bind the port
                if pm_startup.detect_external_ha().await {
                    println!("[HA] stale instance found on :8123 — killing before managed start");
                    services::process_manager::kill_process_on_port_8123();
                }
                if let Err(e) = pm_startup.start_ha().await {
                    eprintln!("Failed to start Home Assistant: {}", e);
                }

                // Populate device cache and start WebSocket subscription
                if ha_client_setup.is_healthy().await {
                    if let Ok(states) = ha_client_setup.get_all_states().await {
                        device_cache_setup.populate(states).await;
                    }

                    // Subscribe to real-time state changes via WebSocket
                    let ws_client = services::ha_ws::HaWebSocketClient::new(&ha_url, &ha_token);
                    let ws_cache = device_cache_setup.clone();
                    match ws_client.subscribe(Arc::new(move |event| {
                        let cache = ws_cache.clone();
                        tokio::spawn(async move {
                            cache.update_entity(&event.entity_id, event.new_state).await;
                        });
                    })).await {
                        Ok(_handle) => println!("[ha-ws] Subscribed to real-time state changes"),
                        Err(e) => eprintln!("[ha-ws] Failed to subscribe: {}", e),
                    }
                }
            });

            // Start MCP server on port 3001
            tauri::async_runtime::spawn(async move {
                if let Err(e) = mcp_server::start_mcp_server(mcp_state, 3001).await {
                    eprintln!("MCP server error: {}", e);
                }
            });

            // Start health monitoring for managed processes
            let pm_monitor = process_manager.clone();
            tauri::async_runtime::spawn(async move {
                pm_monitor.start_monitoring();
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if let Some(pm) = window.try_state::<Arc<ProcessManager>>() {
                    let pm = pm.inner().clone();
                    // Block briefly to ensure processes are stopped
                    tauri::async_runtime::block_on(async {
                        pm.shutdown_all().await;
                    });
                }
            }
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
            commands::setup::check_dependencies,
            commands::setup::install_ollama,
            commands::setup::install_python,
            commands::setup::install_rust,
            commands::setup::install_home_assistant,
            commands::setup::pull_model,
            commands::setup::get_service_status,
            commands::setup::restart_service,
            commands::setup::start_services,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

