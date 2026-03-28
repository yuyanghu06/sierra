use crate::devices::{DeviceInfo, RoomInfo};
use crate::services::ha_client::EntityState;
use crate::state::AppState;

#[tauri::command]
pub async fn get_all_devices(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<DeviceInfo>, String> {
    Ok(state.device_cache.get_all_devices().await)
}

#[tauri::command]
pub async fn get_device_state(
    state: tauri::State<'_, AppState>,
    entity_id: String,
) -> Result<DeviceInfo, String> {
    state
        .device_cache
        .get_device(&entity_id)
        .await
        .ok_or_else(|| format!("Device not found: {}", entity_id))
}

#[tauri::command]
pub async fn get_rooms(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<RoomInfo>, String> {
    Ok(state.device_cache.get_rooms().await)
}

#[tauri::command]
pub async fn get_device_count(
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    Ok(state.device_cache.device_count().await)
}

#[tauri::command]
pub async fn call_device_action(
    state: tauri::State<'_, AppState>,
    domain: String,
    service: String,
    entity_id: String,
    data: Option<serde_json::Value>,
) -> Result<(), String> {
    state
        .ha
        .call_service(&domain, &service, &entity_id, data)
        .await
}

#[tauri::command]
pub async fn check_ha_health(
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    Ok(state.ha.is_healthy().await)
}

#[tauri::command]
pub async fn refresh_devices(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<DeviceInfo>, String> {
    let states = state.ha.get_all_states().await?;
    state.device_cache.populate(states).await;
    Ok(state.device_cache.get_all_devices().await)
}
