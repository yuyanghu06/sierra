use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::Manager;

use crate::services::installer::{self, DependencyStatus, InstallProgress, PullProgress};
use crate::services::process_manager::{ProcessManager, ServiceStatus};

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatusInfo {
    pub ollama: ServiceStatus,
    pub home_assistant: ServiceStatus,
}

#[tauri::command]
pub async fn check_dependencies(app_handle: tauri::AppHandle) -> Result<DependencyStatus, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    Ok(installer::detect_dependencies(&app_data_dir))
}

#[tauri::command]
pub async fn install_ollama(on_progress: Channel<InstallProgress>) -> Result<(), String> {
    installer::install_ollama(&on_progress).await
}

#[tauri::command]
pub async fn install_home_assistant(
    app_handle: tauri::AppHandle,
    on_progress: Channel<InstallProgress>,
) -> Result<(), String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    installer::install_home_assistant(&app_data_dir, &on_progress).await
}

#[tauri::command]
pub async fn pull_model(
    model_name: String,
    on_progress: Channel<PullProgress>,
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<(), String> {
    let ollama_url = state
        .config
        .read()
        .await
        .ollama_url
        .clone()
        .unwrap_or_else(|| "http://localhost:11434".to_string());

    installer::pull_model(&model_name, &ollama_url, &on_progress).await
}

#[tauri::command]
pub async fn get_service_status(
    pm: tauri::State<'_, Arc<ProcessManager>>,
) -> Result<ServiceStatusInfo, String> {
    Ok(ServiceStatusInfo {
        ollama: pm.get_status("ollama").await,
        home_assistant: pm.get_status("home_assistant").await,
    })
}

#[tauri::command]
pub async fn restart_service(
    service: String,
    pm: tauri::State<'_, Arc<ProcessManager>>,
) -> Result<(), String> {
    pm.restart_service(&service).await
}

#[tauri::command]
pub async fn start_services(
    pm: tauri::State<'_, Arc<ProcessManager>>,
) -> Result<ServiceStatusInfo, String> {
    // Detect external instances first
    let ollama_external = pm.detect_external_ollama().await;
    let ha_external = pm.detect_external_ha().await;

    if ollama_external {
        let mut svc = pm.ollama.write().await;
        svc.is_external = true;
        svc.status = ServiceStatus::External;
    } else {
        // Try to start Ollama
        match pm.start_ollama().await {
            Ok(()) => {}
            Err(e) => eprintln!("Failed to start Ollama: {}", e),
        }
    }

    if ha_external {
        let mut svc = pm.home_assistant.write().await;
        svc.is_external = true;
        svc.status = ServiceStatus::External;
    } else {
        // Try to start HA
        match pm.start_ha().await {
            Ok(()) => {}
            Err(e) => eprintln!("Failed to start HA: {}", e),
        }
    }

    Ok(ServiceStatusInfo {
        ollama: pm.get_status("ollama").await,
        home_assistant: pm.get_status("home_assistant").await,
    })
}
