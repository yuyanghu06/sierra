mod commands;
mod services;
mod state;

use services::ollama::OllamaService;
use state::AppState;
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let ollama = OllamaService::new(
        "http://localhost:11434".to_string(),
        "qwen3.5:4b".to_string(),
    );

    tauri::Builder::default()
        .manage(AppState {
            conversation: Mutex::new(Vec::new()),
            llm: Box::new(ollama),
        })
        .invoke_handler(tauri::generate_handler![
            commands::system::ping,
            commands::system::get_app_info,
            commands::chat::send_chat_message,
            commands::chat::clear_conversation,
            commands::chat::check_ollama_health,
            commands::chat::list_models,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
