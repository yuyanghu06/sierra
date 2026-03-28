use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub ha_url: Option<String>,
    pub ha_token: Option<String>,
    pub ollama_url: Option<String>,
    pub ollama_model: Option<String>,
}

fn config_path(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app_handle
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to get config dir: {}", e))?;
    Ok(dir.join("config.json"))
}

pub fn load_config(app_handle: &tauri::AppHandle) -> AppConfig {
    let path = match config_path(app_handle) {
        Ok(p) => p,
        Err(_) => return AppConfig::default(),
    };
    if !path.exists() {
        return AppConfig::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save_config(app_handle: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let path = config_path(app_handle)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config dir: {}", e))?;
    }
    let json =
        serde_json::to_string_pretty(config).map_err(|e| format!("Failed to serialize: {}", e))?;
    std::fs::write(&path, json).map_err(|e| format!("Failed to write config: {}", e))?;

    // Restrict permissions on macOS/Linux
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        let _ = std::fs::set_permissions(&path, perms);
    }

    Ok(())
}
