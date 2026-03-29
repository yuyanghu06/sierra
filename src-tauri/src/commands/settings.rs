use std::sync::Arc;

use crate::config::AppConfig;
use crate::services::ha_client::{HaConnectionStatus, HaRestClient};
use crate::services::ollama::OllamaService;
use crate::services::tool_executor::HaToolExecutor;
use crate::state::AppState;

#[tauri::command]
pub async fn get_config(state: tauri::State<'_, AppState>) -> Result<AppConfig, String> {
    Ok(state.config.read().await.clone())
}

#[tauri::command]
pub async fn save_config(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    config: AppConfig,
) -> Result<(), String> {
    let old_config = state.config.read().await.clone();

    // Persist to disk
    crate::config::save_config(&app_handle, &config)?;

    // Reconfigure HA if URL or token changed
    let ha_changed = config.ha_url != old_config.ha_url || config.ha_token != old_config.ha_token;
    if ha_changed {
        let ha_url = config
            .ha_url
            .clone()
            .unwrap_or_else(|| "http://localhost:8123".to_string());
        let ha_token = config.ha_token.clone().unwrap_or_default();

        let new_ha: Arc<dyn crate::services::ha_client::HomeAssistantService> =
            Arc::new(HaRestClient::new(ha_url, ha_token));

        // Refresh device cache with new HA client
        if new_ha.is_healthy().await {
            if let Ok(states) = new_ha.get_all_states().await {
                state.device_cache.populate(states).await;
            }
        }

        let new_executor: Arc<dyn crate::services::llm::ToolExecutor> =
            Arc::new(HaToolExecutor::new(new_ha.clone()));

        *state.ha.write().await = new_ha;
        *state.tool_executor.write().await = new_executor;
    }

    // Reconfigure Ollama if URL or model changed
    let ollama_changed =
        config.ollama_url != old_config.ollama_url || config.ollama_model != old_config.ollama_model;
    if ollama_changed {
        let ollama_url = config
            .ollama_url
            .clone()
            .unwrap_or_else(|| "http://localhost:11434".to_string());
        let ollama_model = config
            .ollama_model
            .clone()
            .unwrap_or_else(|| "qwen3.5:4b".to_string());

        let new_ollama = OllamaService::new(ollama_url, ollama_model);
        *state.llm.write().await = Box::new(new_ollama);
    }

    // Update stored config
    *state.config.write().await = config;

    Ok(())
}

#[tauri::command]
pub async fn test_ha_connection(url: String, token: String) -> Result<HaConnectionStatus, String> {
    let client = HaRestClient::new(url, token);
    Ok(client.check_connection().await)
}

#[tauri::command]
pub async fn get_active_model(state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    Ok(state.config.read().await.ollama_model.clone())
}
