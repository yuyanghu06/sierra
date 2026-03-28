use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;
use tauri::ipc::Channel;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyStatus {
    pub ollama_installed: bool,
    pub ollama_version: Option<String>,
    pub home_assistant_installed: bool,
    pub ha_version: Option<String>,
    pub python_available: bool,
    pub python_version: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "data")]
pub enum InstallProgress {
    Started { service: String },
    Downloading { percent: f32 },
    Installing,
    Configuring,
    Completed,
    Failed { error: String },
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "data")]
pub enum PullProgress {
    Downloading { percent: f32, total_bytes: u64 },
    Verifying,
    Completed,
    Failed { error: String },
}

/// Detect what's installed on this system
pub fn detect_dependencies(app_data_dir: &PathBuf) -> DependencyStatus {
    let (ollama_installed, ollama_version) = detect_ollama();
    let (ha_installed, ha_version) = detect_ha(app_data_dir);
    let (python_available, python_version) = detect_python();

    DependencyStatus {
        ollama_installed,
        ollama_version,
        home_assistant_installed: ha_installed,
        ha_version,
        python_available,
        python_version,
    }
}

fn detect_ollama() -> (bool, Option<String>) {
    let result = Command::new("ollama").arg("--version").output();

    match result {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // Also check stderr since some versions print there
            let version = if version.is_empty() {
                String::from_utf8_lossy(&output.stderr).trim().to_string()
            } else {
                version
            };
            (true, if version.is_empty() { None } else { Some(version) })
        }
        _ => {
            // Check common install locations
            #[cfg(target_os = "macos")]
            {
                let paths = [
                    "/usr/local/bin/ollama",
                    "/opt/homebrew/bin/ollama",
                ];
                for p in &paths {
                    if PathBuf::from(p).exists() {
                        return (true, None);
                    }
                }
            }
            #[cfg(target_os = "windows")]
            {
                if let Ok(local) = std::env::var("LOCALAPPDATA") {
                    let p = PathBuf::from(&local).join("Programs\\Ollama\\ollama.exe");
                    if p.exists() {
                        return (true, None);
                    }
                }
            }
            (false, None)
        }
    }
}

fn detect_ha(app_data_dir: &PathBuf) -> (bool, Option<String>) {
    let venv = app_data_dir.join("ha-venv");

    #[cfg(target_os = "macos")]
    let hass = venv.join("bin/hass");
    #[cfg(target_os = "windows")]
    let hass = venv.join("Scripts\\hass.exe");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let hass = venv.join("bin/hass");

    if !hass.exists() {
        return (false, None);
    }

    let result = Command::new(&hass).arg("--version").output();
    match result {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (true, if version.is_empty() { None } else { Some(version) })
        }
        _ => (true, None), // Binary exists even if --version fails
    }
}

fn detect_python() -> (bool, Option<String>) {
    // Try python3 first (macOS), then python (Windows)
    let commands = if cfg!(target_os = "windows") {
        vec!["python", "python3"]
    } else {
        vec!["python3", "python"]
    };

    for cmd in commands {
        if let Ok(output) = Command::new(cmd).arg("--version").output() {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let version = if version.is_empty() {
                    String::from_utf8_lossy(&output.stderr).trim().to_string()
                } else {
                    version
                };
                return (true, if version.is_empty() { None } else { Some(version) });
            }
        }
    }
    (false, None)
}

/// Get the python command name for this platform
fn python_cmd() -> &'static str {
    if cfg!(target_os = "windows") {
        "python"
    } else {
        "python3"
    }
}

/// Install Ollama
pub async fn install_ollama(on_progress: &Channel<InstallProgress>) -> Result<(), String> {
    let _ = on_progress.send(InstallProgress::Started {
        service: "Ollama".to_string(),
    });

    #[cfg(target_os = "macos")]
    {
        let _ = on_progress.send(InstallProgress::Downloading { percent: 0.0 });

        // Download Ollama install script and run it
        // The official install script handles architecture detection
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg("curl -fsSL https://ollama.com/install.sh | sh")
            .output()
            .await
            .map_err(|e| format!("Failed to run install script: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let _ = on_progress.send(InstallProgress::Failed {
                error: stderr.clone(),
            });
            return Err(format!("Ollama installation failed: {}", stderr));
        }

        let _ = on_progress.send(InstallProgress::Completed);
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        let _ = on_progress.send(InstallProgress::Downloading { percent: 0.0 });

        // Download the installer
        let client = reqwest::Client::new();
        let temp_dir = std::env::temp_dir();
        let installer_path = temp_dir.join("OllamaSetup.exe");

        let response = client
            .get("https://ollama.com/download/OllamaSetup.exe")
            .send()
            .await
            .map_err(|e| format!("Failed to download Ollama: {}", e))?;

        let total = response.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;
        let mut bytes = Vec::new();

        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Download error: {}", e))?;
            downloaded += chunk.len() as u64;
            bytes.extend_from_slice(&chunk);
            if total > 0 {
                let percent = (downloaded as f32 / total as f32) * 100.0;
                let _ = on_progress.send(InstallProgress::Downloading { percent });
            }
        }

        std::fs::write(&installer_path, &bytes)
            .map_err(|e| format!("Failed to write installer: {}", e))?;

        let _ = on_progress.send(InstallProgress::Installing);

        // Run silent install
        let output = tokio::process::Command::new(&installer_path)
            .args(["/SILENT", "/NORESTART"])
            .output()
            .await
            .map_err(|e| format!("Failed to run installer: {}", e))?;

        // Clean up installer
        let _ = std::fs::remove_file(&installer_path);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let _ = on_progress.send(InstallProgress::Failed {
                error: stderr.clone(),
            });
            return Err(format!("Ollama installation failed: {}", stderr));
        }

        let _ = on_progress.send(InstallProgress::Completed);
        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = on_progress.send(InstallProgress::Failed {
            error: "Platform not supported".to_string(),
        });
        Err("Platform not supported for automatic Ollama installation".to_string())
    }
}

/// Install Home Assistant Core in a managed venv
pub async fn install_home_assistant(
    app_data_dir: &PathBuf,
    on_progress: &Channel<InstallProgress>,
) -> Result<(), String> {
    let _ = on_progress.send(InstallProgress::Started {
        service: "Home Assistant".to_string(),
    });

    // Check Python is available
    let python = python_cmd();
    let python_check = tokio::process::Command::new(python)
        .arg("--version")
        .output()
        .await;

    if python_check.is_err() || !python_check.unwrap().status.success() {
        let _ = on_progress.send(InstallProgress::Failed {
            error: "Python 3 is required but not found. Please install Python 3.12+.".to_string(),
        });
        return Err("Python 3 not found".to_string());
    }

    let venv_path = app_data_dir.join("ha-venv");

    // Step 1: Create virtual environment
    let _ = on_progress.send(InstallProgress::Installing);

    let output = tokio::process::Command::new(python)
        .args(["-m", "venv", venv_path.to_str().unwrap()])
        .output()
        .await
        .map_err(|e| format!("Failed to create venv: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let _ = on_progress.send(InstallProgress::Failed {
            error: stderr.clone(),
        });
        return Err(format!("Failed to create venv: {}", stderr));
    }

    // Step 2: Install homeassistant
    let _ = on_progress.send(InstallProgress::Downloading { percent: 50.0 });

    #[cfg(target_os = "macos")]
    let pip = venv_path.join("bin/pip");
    #[cfg(target_os = "windows")]
    let pip = venv_path.join("Scripts\\pip.exe");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let pip = venv_path.join("bin/pip");

    let output = tokio::process::Command::new(&pip)
        .args(["install", "homeassistant"])
        .output()
        .await
        .map_err(|e| format!("Failed to install homeassistant: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let _ = on_progress.send(InstallProgress::Failed {
            error: stderr.clone(),
        });
        return Err(format!("Failed to install homeassistant: {}", stderr));
    }

    // Step 3: Pin pycares to avoid DNS resolver errors
    let _ = on_progress.send(InstallProgress::Configuring);

    let output = tokio::process::Command::new(&pip)
        .args(["install", "pycares==4.11.0"])
        .output()
        .await
        .map_err(|e| format!("Failed to pin pycares: {}", e))?;

    if !output.status.success() {
        // Non-fatal — log but continue
        eprintln!(
            "Warning: Failed to pin pycares: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Create HA config directory
    let ha_config = app_data_dir.join("ha-config");
    let _ = std::fs::create_dir_all(&ha_config);

    let _ = on_progress.send(InstallProgress::Completed);
    Ok(())
}

/// Pull a model through Ollama's API with progress streaming
pub async fn pull_model(
    model_name: &str,
    ollama_url: &str,
    on_progress: &Channel<PullProgress>,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3600)) // 1 hour timeout for large models
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .post(format!("{}/api/pull", ollama_url))
        .json(&serde_json::json!({ "name": model_name }))
        .send()
        .await
        .map_err(|e| format!("Failed to start model pull: {}", e))?;

    if !response.status().is_success() {
        let body = response.text().await.unwrap_or_default();
        let _ = on_progress.send(PullProgress::Failed {
            error: body.clone(),
        });
        return Err(format!("Ollama pull failed: {}", body));
    }

    // Stream NDJSON progress
    use futures_util::StreamExt;
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(item) = stream.next().await {
        let bytes = item.map_err(|e| format!("Stream error: {}", e))?;
        let text = std::str::from_utf8(&bytes).map_err(|e| format!("UTF-8 error: {}", e))?;
        buffer.push_str(text);

        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                let status = json.get("status").and_then(|s| s.as_str()).unwrap_or("");

                if status.contains("pulling") || status.contains("downloading") {
                    let total = json.get("total").and_then(|t| t.as_u64()).unwrap_or(0);
                    let completed = json.get("completed").and_then(|c| c.as_u64()).unwrap_or(0);
                    let percent = if total > 0 {
                        (completed as f32 / total as f32) * 100.0
                    } else {
                        0.0
                    };
                    let _ = on_progress.send(PullProgress::Downloading {
                        percent,
                        total_bytes: total,
                    });
                } else if status.contains("verifying") {
                    let _ = on_progress.send(PullProgress::Verifying);
                } else if status == "success" {
                    let _ = on_progress.send(PullProgress::Completed);
                    return Ok(());
                }

                // Check for error
                if let Some(error) = json.get("error").and_then(|e| e.as_str()) {
                    let _ = on_progress.send(PullProgress::Failed {
                        error: error.to_string(),
                    });
                    return Err(error.to_string());
                }
            }
        }
    }

    let _ = on_progress.send(PullProgress::Completed);
    Ok(())
}
