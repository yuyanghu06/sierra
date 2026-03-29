use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;
use tauri::ipc::Channel;

use crate::util::HideConsole;

/// Returns the space-free path used for the managed HA virtual environment.
///
/// uv (bundled with HA) has a bug in >=0.6.x where it splits `--constraint`
/// paths on spaces, causing startup failures when the app data directory
/// contains "Application Support". Using a home-relative path avoids this.
pub fn ha_venv_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(base).join(".sierra").join("ha-venv")
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".sierra").join("ha-venv")
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyStatus {
    pub ollama_installed: bool,
    pub ollama_version: Option<String>,
    pub home_assistant_installed: bool,
    pub ha_version: Option<String>,
    pub python_available: bool,
    pub python_version: Option<String>,
    pub rust_available: bool,
    pub rust_version: Option<String>,
    /// Windows only: whether WSL is installed and a default distro is available.
    /// Always `true` on macOS/Linux (WSL is not applicable).
    pub wsl_available: bool,
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
    let (rust_available, rust_version) = detect_rust();

    DependencyStatus {
        ollama_installed,
        ollama_version,
        home_assistant_installed: ha_installed,
        ha_version,
        python_available,
        python_version,
        rust_available,
        rust_version,
        wsl_available: detect_wsl(),
    }
}

/// Returns `true` if WSL is installed and usable. Always `true` on non-Windows.
pub fn detect_wsl() -> bool {
    #[cfg(not(target_os = "windows"))]
    return true;

    #[cfg(target_os = "windows")]
    {
        // `wsl -- echo ok` succeeds only when WSL is installed and a default distro exists.
        Command::new("wsl")
            .args(["--", "echo", "ok"])
            .hide_console()
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

fn detect_ollama() -> (bool, Option<String>) {
    let result = Command::new("ollama").arg("--version").hide_console().output();

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

fn detect_ha(_app_data_dir: &PathBuf) -> (bool, Option<String>) {
    // On Windows, HA runs inside WSL — check there instead of native PATH.
    // hass.exe from a Windows-native venv will always refuse to start:
    // "Home Assistant only supports Linux, OSX and Windows using WSL"
    #[cfg(target_os = "windows")]
    {
        let check = Command::new("wsl")
            .args(["--", "bash", "-c", "test -f ~/.sierra/ha-venv/bin/hass"])
            .hide_console()
            .output();
        return match check {
            Ok(output) => (output.status.success(), None),
            Err(_) => (false, None),
        };
    }

    // macOS / Linux: check managed venv first, then system PATH.
    #[cfg(not(target_os = "windows"))]
    {
        let venv = ha_venv_dir();
        let hass = venv.join("bin/hass");

        if hass.exists() {
            let result = Command::new(&hass).arg("--version").output();
            return match result {
                Ok(output) if output.status.success() => {
                    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    (true, if version.is_empty() { None } else { Some(version) })
                }
                _ => (true, None),
            };
        }

        if let Ok(output) = Command::new("which").arg("hass").output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !path.is_empty() {
                    if let Ok(ver_output) = Command::new(&path).arg("--version").output() {
                        if ver_output.status.success() {
                            let version =
                                String::from_utf8_lossy(&ver_output.stdout).trim().to_string();
                            return (true, if version.is_empty() { None } else { Some(version) });
                        }
                    }
                    return (true, None);
                }
            }
        }

        (false, None)
    }
}

fn detect_python() -> (bool, Option<String>) {
    if let Some(cmd) = find_python_3_10_plus() {
        // `find_python_3_10_plus` may return "py:-3.X" on Windows (py launcher + version flag).
        // Split that into binary + args before invoking; `Command::new("py:-3.13")` would fail.
        let output = if let Some((launcher, ver_flag)) = cmd.split_once(':') {
            Command::new(launcher)
                .args([ver_flag, "--version"])
                .hide_console()
                .output()
        } else {
            Command::new(&cmd).arg("--version").hide_console().output()
        };

        if let Ok(output) = output {
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

fn detect_rust() -> (bool, Option<String>) {
    // Try rustc on PATH first
    if let Ok(output) = Command::new("rustc").arg("--version").hide_console().output() {
        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return (true, if version.is_empty() { None } else { Some(version) });
        }
    }

    // Check ~/.cargo/bin/rustc which rustup installs by default
    #[cfg(target_os = "windows")]
    let rustc_path = std::env::var("USERPROFILE")
        .map(|home| PathBuf::from(home).join(".cargo").join("bin").join("rustc.exe"))
        .ok();
    #[cfg(not(target_os = "windows"))]
    let rustc_path = std::env::var("HOME")
        .map(|home| PathBuf::from(home).join(".cargo").join("bin").join("rustc"))
        .ok();

    if let Some(path) = rustc_path {
        if path.exists() {
            if let Ok(output) = Command::new(&path).arg("--version").hide_console().output() {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    return (true, if version.is_empty() { None } else { Some(version) });
                }
            }
            return (true, None); // binary exists, version check failed
        }
    }

    (false, None)
}

/// Find the best available Python >= 3.10 on the system.
/// Prefers specific versioned commands (3.13 → 3.10) before falling back to
/// the generic `python3` / `python` names, which on macOS may point to Apple's
/// bundled Python 3.9 or earlier.
///
/// Also checks well-known absolute paths because Tauri spawns subprocesses with
/// a restricted PATH (/usr/bin:/bin only) that omits Homebrew, pyenv, and the
/// official python.org installer locations.
fn find_python_3_10_plus() -> Option<String> {
    // Short names tried via PATH (works in dev / terminal context).
    // On Windows also try the `py` launcher which is installed by the python.org
    // installer and supports versioned invocation.
    let name_candidates: Vec<&str> = if cfg!(target_os = "windows") {
        vec![
            "python3.13", "python3.12", "python3.11", "python3.10",
            "python", "python3",
        ]
    } else {
        vec![
            "python3.13", "python3.12", "python3.11", "python3.10",
            "python3", "python",
        ]
    };

    // Absolute paths to check — covers macOS python.org, Homebrew (Intel + ARM),
    // pyenv default, and Windows py launcher / common install dirs.
    #[cfg(target_os = "macos")]
    let abs_candidates: Vec<String> = {
        let mut paths = Vec::new();
        for minor in (10u32..=15).rev() {
            let ver = format!("3.{}", minor);
            // python.org framework installer
            paths.push(format!(
                "/Library/Frameworks/Python.framework/Versions/{}/bin/python{}",
                ver, ver
            ));
            // Homebrew Apple Silicon
            paths.push(format!("/opt/homebrew/bin/python{}", ver));
            // Homebrew Intel / /usr/local
            paths.push(format!("/usr/local/bin/python{}", ver));
        }
        // pyenv shims
        if let Ok(home) = std::env::var("HOME") {
            for minor in (10u32..=15).rev() {
                paths.push(format!("{}/.pyenv/shims/python3.{}", home, minor));
            }
        }
        paths
    };

    #[cfg(target_os = "windows")]
    let abs_candidates: Vec<String> = {
        let mut paths = Vec::new();

        // Windows `py` launcher — installed by python.org and supports versioned flags.
        // We check if `py -3.X --version` works by probing via the launcher name.
        // (Actual invocation happens below using the launcher path after detection.)
        let py_launcher = r"C:\Windows\py.exe";
        for minor in (10u32..=15).rev() {
            // Launcher-style: "py -3.13" — encode as a wrapper path we can probe
            // We handle this specially in the probe loop below.
            paths.push(format!("py:-3.{}", minor));
        }

        // python.org default install: C:\Users\<user>\AppData\Local\Programs\Python\Python3XX
        // Use LOCALAPPDATA env var — handles non-C: drives and domain accounts.
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            for minor in (10u32..=15).rev() {
                paths.push(format!(
                    "{}\\Programs\\Python\\Python3{}\\python.exe",
                    local, minor
                ));
            }
        }

        // Classic per-machine install: C:\Python3XX (still common)
        let sys_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
        for minor in (10u32..=15).rev() {
            paths.push(format!("{}\\Python3{}\\python.exe", sys_drive, minor));
        }

        // py launcher absolute path fallback (if not on PATH)
        if std::path::Path::new(py_launcher).exists() {
            for minor in (10u32..=15).rev() {
                paths.push(format!("{}:-3.{}", py_launcher, minor));
            }
        }

        paths
    };

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let abs_candidates: Vec<String> = (10u32..=15)
        .rev()
        .map(|minor| format!("/usr/bin/python3.{}", minor))
        .collect();

    // Chain name candidates first (respects user PATH in dev), then absolute paths
    let all_candidates: Vec<String> = name_candidates
        .iter()
        .map(|s| s.to_string())
        .chain(abs_candidates)
        .collect();

    for cmd in &all_candidates {
        // Handle Windows `py` launcher entries encoded as "path:-3.X" or "py:-3.X"
        #[cfg(target_os = "windows")]
        if let Some((launcher, ver_flag)) = cmd.split_once(':') {
            let launcher_bin = if launcher == "py" { "py" } else { launcher };
            if let Ok(output) = Command::new(launcher_bin).args([ver_flag, "--version"]).hide_console().output() {
                if output.status.success() {
                    let raw = {
                        let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        if out.is_empty() {
                            String::from_utf8_lossy(&output.stderr).trim().to_string()
                        } else {
                            out
                        }
                    };
                    if let Some(ver) = raw.strip_prefix("Python ") {
                        let parts: Vec<u64> = ver
                            .split('.')
                            .filter_map(|s| s.parse().ok())
                            .collect();
                        if parts.len() >= 2 && (parts[0] > 3 || (parts[0] == 3 && parts[1] >= 10)) {
                            // Return the launcher invocation as a usable command string.
                            // Caller (venv creation) will split on ':' to get args.
                            println!("[installer] found Python >= 3.10 via py launcher: {} {}", launcher_bin, ver_flag);
                            return Some(cmd.clone());
                        }
                    }
                }
            }
            continue;
        }

        if let Ok(output) = Command::new(cmd).arg("--version").hide_console().output() {
            if !output.status.success() {
                continue;
            }
            let raw = {
                let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if out.is_empty() {
                    String::from_utf8_lossy(&output.stderr).trim().to_string()
                } else {
                    out
                }
            };
            if let Some(ver) = raw.strip_prefix("Python ") {
                let parts: Vec<u64> = ver
                    .split('.')
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if parts.len() >= 2 && (parts[0] > 3 || (parts[0] == 3 && parts[1] >= 10)) {
                    println!("[installer] found Python >= 3.10 at: {}", cmd);
                    return Some(cmd.clone());
                }
            }
        }
    }
    None
}

/// Fetch the latest published Ollama version tag from GitHub releases.
/// Returns `None` if the network request fails (non-fatal).
pub async fn fetch_latest_ollama_version() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("sierra-app")
        .build()
        .ok()?;

    let resp = client
        .get("https://api.github.com/repos/ollama/ollama/releases/latest")
        .send()
        .await
        .ok()?;

    let json: serde_json::Value = resp.json().await.ok()?;
    let tag = json["tag_name"].as_str()?;
    // Tags are "v0.x.x" — strip the leading 'v'
    Some(tag.trim_start_matches('v').to_string())
}

/// Parse a version string like "ollama version is 0.6.5" or "0.6.5" into (major, minor, patch).
fn parse_ollama_version(s: &str) -> Option<(u32, u32, u32)> {
    let s = s.to_lowercase();
    let s = s.trim_start_matches("ollama version is ").trim();
    let s = s.trim_start_matches('v');
    let mut parts = s.splitn(3, '.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next().unwrap_or("0").split_whitespace().next()?.parse().ok()?;
    Some((major, minor, patch))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaUpdateStatus {
    pub update_available: bool,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
}

/// Check whether the installed Ollama is older than the latest GitHub release.
pub async fn check_ollama_update() -> OllamaUpdateStatus {
    let (installed, current_version) = detect_ollama();

    if !installed {
        return OllamaUpdateStatus {
            update_available: false,
            current_version: None,
            latest_version: None,
        };
    }

    let latest_version = fetch_latest_ollama_version().await;

    let update_available = match (&current_version, &latest_version) {
        (Some(cur), Some(lat)) => {
            match (parse_ollama_version(cur), parse_ollama_version(lat)) {
                (Some(cv), Some(lv)) => lv > cv,
                _ => false,
            }
        }
        _ => false,
    };

    OllamaUpdateStatus {
        update_available,
        current_version,
        latest_version,
    }
}

/// Install WSL (Windows only). Launches `wsl --install` elevated via PowerShell.
/// A reboot may be required after installation completes.
/// On non-Windows this is a no-op that immediately returns `Ok(())`.
pub async fn install_wsl(on_progress: &Channel<InstallProgress>) -> Result<(), String> {
    let _ = on_progress.send(InstallProgress::Started {
        service: "WSL".to_string(),
    });

    #[cfg(not(target_os = "windows"))]
    {
        let _ = on_progress.send(InstallProgress::Completed);
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        let _ = on_progress.send(InstallProgress::Installing);

        // Run `wsl --install` elevated through PowerShell so the UAC prompt appears.
        // `-Wait` blocks until installation finishes before returning.
        let output = tokio::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Process -FilePath 'wsl' -ArgumentList '--install' -Verb RunAs -Wait",
            ])
            .hide_console()
            .output()
            .await
            .map_err(|e| format!("Failed to launch WSL installer: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let _ = on_progress.send(InstallProgress::Failed { error: stderr.clone() });
            return Err(format!("WSL installation failed: {}", stderr));
        }

        let _ = on_progress.send(InstallProgress::Completed);
        Ok(())
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
            .hide_console()
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

/// Install Home Assistant Core in a managed venv.
///
/// On Windows HA cannot run natively — it requires WSL. This function installs
/// HA inside the default WSL distribution's filesystem and is a no-op if WSL
/// is unavailable. On macOS/Linux it uses a local Python venv as before.
pub async fn install_home_assistant(
    _app_data_dir: &PathBuf,
    _resource_dir: &PathBuf,
    on_progress: &Channel<InstallProgress>,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    return install_home_assistant_wsl(on_progress).await;

    #[cfg(not(target_os = "windows"))]
    install_home_assistant_native(_app_data_dir, _resource_dir, on_progress).await
}

/// Windows: install HA inside WSL. WSL auto-forwards port 8123 to the Windows host.
#[cfg(target_os = "windows")]
async fn install_home_assistant_wsl(on_progress: &Channel<InstallProgress>) -> Result<(), String> {
    let _ = on_progress.send(InstallProgress::Started {
        service: "Home Assistant".to_string(),
    });

    // Verify WSL is available
    let wsl_ok = tokio::process::Command::new("wsl")
        .args(["--", "echo", "ok"])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !wsl_ok {
        let err = "WSL (Windows Subsystem for Linux) is required to run Home Assistant on Windows. \
                   Install it by running: wsl --install".to_string();
        let _ = on_progress.send(InstallProgress::Failed { error: err.clone() });
        return Err(err);
    }

    // Check if python3 venv support is already available. If not, install
    // python3.12-venv via apt. Force IPv4 to avoid hangs on systems where DNS
    // returns IPv6 addresses but IPv6 connectivity is unavailable (e.g. WSL
    // behind Tailscale). Skip apt entirely if venv already works — avoids
    // blocking on apt locks left by previous runs or other processes.
    let venv_check = tokio::process::Command::new("wsl")
        .args(["--", "bash", "-c", "python3 -m venv --help > /dev/null 2>&1"])
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !venv_check {
        let _ = on_progress.send(InstallProgress::Installing);

        let apt_script = "\
            sudo apt-get update -qq -o Acquire::ForceIPv4=true 2>/dev/null && \
            sudo apt-get install -y -qq -o Acquire::ForceIPv4=true python3.12-venv 2>/dev/null";

        let apt_out = tokio::process::Command::new("wsl")
            .args(["--", "bash", "-c", apt_script])
            .output()
            .await
            .map_err(|e| format!("Failed to install python3.12-venv: {}", e))?;

        if !apt_out.status.success() {
            let stderr = String::from_utf8_lossy(&apt_out.stderr).to_string();
            let _ = on_progress.send(InstallProgress::Failed { error: stderr.clone() });
            return Err(format!("apt install failed: {}", stderr));
        }
    }

    // Create the venv and install HA. Pin pycares to avoid aiodns/DNS resolver
    // errors on some Linux kernels.
    let install_script = "\
        set -e && \
        python3 -m venv ~/.sierra/ha-venv && \
        ~/.sierra/ha-venv/bin/pip install --upgrade pip --quiet && \
        ~/.sierra/ha-venv/bin/pip install 'pycares==4.11.0' homeassistant --quiet";

    let _ = on_progress.send(InstallProgress::Downloading { percent: 20.0 });

    let output = tokio::process::Command::new("wsl")
        .args(["--", "bash", "-c", install_script])
        .output()
        .await
        .map_err(|e| format!("Failed to run WSL install: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let _ = on_progress.send(InstallProgress::Failed { error: stderr.clone() });
        return Err(format!("HA install in WSL failed: {}", stderr));
    }

    let _ = on_progress.send(InstallProgress::Configuring);

    // Ensure config directory exists in WSL filesystem
    let _ = tokio::process::Command::new("wsl")
        .args(["--", "bash", "-c", "mkdir -p ~/.sierra/ha-config"])
        .output()
        .await;

    let _ = on_progress.send(InstallProgress::Completed);
    Ok(())
}

/// macOS / Linux: install HA in a local Python venv.
#[cfg(not(target_os = "windows"))]
async fn install_home_assistant_native(
    app_data_dir: &PathBuf,
    resource_dir: &PathBuf,
    on_progress: &Channel<InstallProgress>,
) -> Result<(), String> {
    let _ = on_progress.send(InstallProgress::Started {
        service: "Home Assistant".to_string(),
    });

    // Find a Python >= 3.10 — required by Home Assistant and its dependencies.
    // Prefer versioned commands (python3.13, python3.12, ...) over the generic
    // `python3` which on macOS may be Apple's ancient bundled Python < 3.10.
    let python = match find_python_3_10_plus() {
        Some(cmd) => cmd,
        None => {
            let _ = on_progress.send(InstallProgress::Failed {
                error: "Python 3.10 or newer is required but not found. Please install Python 3.12+ from python.org.".to_string(),
            });
            return Err("Python 3.10+ not found".to_string());
        }
    };
    println!("[installer] using Python: {}", python);

    let venv_path = ha_venv_dir();

    // Step 1: Remove any stale venv, then create a fresh one with Python >= 3.10.
    // A venv created with an older Python (< 3.10) will silently fail to install
    // many of HA's dependencies, so we always start clean.
    if venv_path.exists() {
        println!("[installer] removing stale venv at {}", venv_path.display());
        if let Err(e) = std::fs::remove_dir_all(&venv_path) {
            eprintln!("[installer] warning: could not remove stale venv: {}", e);
        }
    }
    let _ = on_progress.send(InstallProgress::Installing);

    // Build the venv creation command — handle Windows `py:-3.X` launcher entries.
    #[cfg(target_os = "windows")]
    let venv_output = if let Some((launcher, ver_flag)) = python.split_once(':') {
        let launcher_bin = if launcher == "py" { "py" } else { launcher };
        tokio::process::Command::new(launcher_bin)
            .args([ver_flag, "-m", "venv", venv_path.to_str().unwrap()])
            .hide_console()
            .output()
            .await
            .map_err(|e| format!("Failed to create venv: {}", e))?
    } else {
        tokio::process::Command::new(&python)
            .args(["-m", "venv", venv_path.to_str().unwrap()])
            .hide_console()
            .output()
            .await
            .map_err(|e| format!("Failed to create venv: {}", e))?
    };
    #[cfg(not(target_os = "windows"))]
    let venv_output = tokio::process::Command::new(&python)
        .args(["-m", "venv", venv_path.to_str().unwrap()])
        .hide_console()
        .output()
        .await
        .map_err(|e| format!("Failed to create venv: {}", e))?;

    let output = venv_output;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let _ = on_progress.send(InstallProgress::Failed {
            error: stderr.clone(),
        });
        return Err(format!("Failed to create venv: {}", stderr));
    }

    #[cfg(target_os = "macos")]
    let pip = venv_path.join("bin").join("pip");
    #[cfg(target_os = "windows")]
    let pip = venv_path.join("Scripts").join("pip.exe");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let pip = venv_path.join("bin").join("pip");

    // Step 2: Upgrade pip inside the venv before installing packages.
    // The pip bundled with freshly-created venvs can be very old (e.g. 21.x)
    // and fails to resolve packages that require newer metadata handling.
    let _ = on_progress.send(InstallProgress::Downloading { percent: 10.0 });

    #[cfg(target_os = "macos")]
    let python_in_venv = venv_path.join("bin").join("python3");
    #[cfg(target_os = "windows")]
    let python_in_venv = venv_path.join("Scripts").join("python.exe");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let python_in_venv = venv_path.join("bin").join("python3");

    let upgrade_output = tokio::process::Command::new(&python_in_venv)
        .args(["-m", "pip", "install", "--upgrade", "pip"])
        .hide_console()
        .output()
        .await
        .map_err(|e| format!("Failed to upgrade pip: {}", e))?;

    if !upgrade_output.status.success() {
        // Non-fatal — log and continue with whatever pip version we have
        let stderr = String::from_utf8_lossy(&upgrade_output.stderr);
        eprintln!("[installer] pip upgrade warning: {}", stderr);
    } else {
        println!("[installer] pip upgraded successfully");
    }

    // Step 3: Install pinned dependencies from bundled requirements.txt
    let _ = on_progress.send(InstallProgress::Downloading { percent: 25.0 });

    // In production builds, resources are in resource_dir.
    // In dev mode, fall back to src-tauri/resources/.
    let requirements_path = {
        let bundled = resource_dir.join("requirements.txt");
        if bundled.exists() {
            bundled
        } else {
            let dev_fallback = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/requirements.txt");
            if dev_fallback.exists() {
                dev_fallback
            } else {
                let _ = on_progress.send(InstallProgress::Failed {
                    error: "Bundled requirements.txt not found".to_string(),
                });
                return Err(format!(
                    "requirements.txt not found at {} or {}",
                    bundled.display(),
                    dev_fallback.display()
                ));
            }
        }
    };
    println!("[installer] Using requirements.txt from {}", requirements_path.display());

    let output = tokio::process::Command::new(&pip)
        .args([
            "install",
            "-r",
            requirements_path.to_str().unwrap(),
        ])
        .hide_console()
        .output()
        .await
        .map_err(|e| format!("Failed to install requirements: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let _ = on_progress.send(InstallProgress::Failed {
            error: stderr.clone(),
        });
        return Err(format!("Failed to install requirements: {}", stderr));
    }

    // Step 4: Install homeassistant into the same venv
    let _ = on_progress.send(InstallProgress::Downloading { percent: 75.0 });

    let output = tokio::process::Command::new(&pip)
        .args(["install", "homeassistant"])
        .hide_console()
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

    // Step 5: Configure
    let _ = on_progress.send(InstallProgress::Configuring);

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

// ── Python installation ──────────────────────────────────────────────────────

/// Python 3.12 installer URLs (python.org official releases).
/// Update this constant when a newer patch release is published.
const PYTHON_VERSION: &str = "3.12.10";
#[allow(dead_code)]
const PYTHON_MACOS_PKG_URL: &str =
    "https://www.python.org/ftp/python/3.12.10/python-3.12.10-macos11.pkg";
#[allow(dead_code)]
const PYTHON_WINDOWS_URL: &str =
    "https://www.python.org/ftp/python/3.12.10/python-3.12.10-amd64.exe";

/// Install Python 3.12 on macOS or Windows.
///
/// macOS: tries Homebrew first (user-space, no admin prompt), then falls back
/// to the official python.org `.pkg` installer (requires admin via a system dialog).
///
/// Windows: downloads and runs the official python.org installer in quiet,
/// per-user mode (no admin required, PATH is updated automatically).
pub async fn install_python(on_progress: &Channel<InstallProgress>) -> Result<(), String> {
    let _ = on_progress.send(InstallProgress::Started {
        service: "Python".to_string(),
    });

    #[cfg(target_os = "macos")]
    {
        // ── Try Homebrew first ──────────────────────────────────────────────
        let brew = find_brew();
        if let Some(brew_bin) = brew {
            let _ = on_progress.send(InstallProgress::Downloading { percent: 10.0 });
            let output = tokio::process::Command::new(&brew_bin)
                .args(["install", "python@3.12"])
                .output()
                .await
                .map_err(|e| format!("Failed to run brew: {}", e))?;

            if output.status.success() {
                let _ = on_progress.send(InstallProgress::Completed);
                return Ok(());
            }
            // brew failed — fall through to pkg installer
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("[installer] brew install python@3.12 failed: {}", stderr);
        }

        // ── Fall back: download python.org .pkg and run with admin privilege ─
        let _ = on_progress.send(InstallProgress::Downloading { percent: 0.0 });

        let client = reqwest::Client::new();
        let temp_dir = std::env::temp_dir();
        let pkg_filename = format!("python-{}-macos11.pkg", PYTHON_VERSION);
        let pkg_path = temp_dir.join(&pkg_filename);

        let response = client
            .get(PYTHON_MACOS_PKG_URL)
            .send()
            .await
            .map_err(|e| format!("Failed to download Python: {}", e))?;

        if !response.status().is_success() {
            let msg = format!("Python download failed (HTTP {})", response.status());
            let _ = on_progress.send(InstallProgress::Failed { error: msg.clone() });
            return Err(msg);
        }

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
                let percent = (downloaded as f32 / total as f32) * 90.0;
                let _ = on_progress.send(InstallProgress::Downloading { percent });
            }
        }

        std::fs::write(&pkg_path, &bytes)
            .map_err(|e| format!("Failed to write Python pkg: {}", e))?;

        let _ = on_progress.send(InstallProgress::Installing);

        // Run the pkg installer with administrator privileges via osascript.
        // This pops up a standard macOS authentication dialog.
        let shell_cmd = format!(
            "installer -pkg {} -target /",
            pkg_path.to_string_lossy()
        );
        let osascript_arg = format!(
            "do shell script \"{}\" with administrator privileges",
            shell_cmd
        );
        let output = tokio::process::Command::new("osascript")
            .args(["-e", &osascript_arg])
            .output()
            .await
            .map_err(|e| format!("Failed to run osascript: {}", e))?;

        // Clean up downloaded pkg regardless of outcome
        let _ = std::fs::remove_file(&pkg_path);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let _ = on_progress.send(InstallProgress::Failed { error: stderr.clone() });
            return Err(format!("Python installation failed: {}", stderr));
        }

        let _ = on_progress.send(InstallProgress::Completed);
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        let _ = on_progress.send(InstallProgress::Downloading { percent: 0.0 });

        let client = reqwest::Client::new();
        let temp_dir = std::env::temp_dir();
        let installer_filename = format!("python-{}-amd64.exe", PYTHON_VERSION);
        let installer_path = temp_dir.join(&installer_filename);

        let response = client
            .get(PYTHON_WINDOWS_URL)
            .send()
            .await
            .map_err(|e| format!("Failed to download Python: {}", e))?;

        if !response.status().is_success() {
            let msg = format!("Python download failed (HTTP {})", response.status());
            let _ = on_progress.send(InstallProgress::Failed { error: msg.clone() });
            return Err(msg);
        }

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
                let percent = (downloaded as f32 / total as f32) * 90.0;
                let _ = on_progress.send(InstallProgress::Downloading { percent });
            }
        }

        std::fs::write(&installer_path, &bytes)
            .map_err(|e| format!("Failed to write Python installer: {}", e))?;

        let _ = on_progress.send(InstallProgress::Installing);

        // Per-user install (no admin required), prepend to PATH so it's usable immediately.
        let output = tokio::process::Command::new(&installer_path)
            .args([
                "/quiet",
                "InstallAllUsers=0",
                "PrependPath=1",
                "Include_test=0",
                "Include_launcher=1",
            ])
            .hide_console()
            .output()
            .await
            .map_err(|e| format!("Failed to run Python installer: {}", e))?;

        // Clean up installer
        let _ = std::fs::remove_file(&installer_path);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let _ = on_progress.send(InstallProgress::Failed { error: stderr.clone() });
            return Err(format!("Python installation failed: {}", stderr));
        }

        let _ = on_progress.send(InstallProgress::Completed);
        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = on_progress.send(InstallProgress::Failed {
            error: "Platform not supported".to_string(),
        });
        Err("Platform not supported for automatic Python installation".to_string())
    }
}

/// Find the Homebrew binary on macOS. Returns the absolute path or "brew" if it is on PATH.
#[cfg(target_os = "macos")]
fn find_brew() -> Option<String> {
    // Common install locations (Homebrew on Apple Silicon vs Intel)
    for path in &["/opt/homebrew/bin/brew", "/usr/local/bin/brew"] {
        if PathBuf::from(path).exists() {
            return Some(path.to_string());
        }
    }
    // Fallback: try PATH (works in dev/terminal context)
    if Command::new("brew")
        .arg("--version")
        .hide_console()
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Some("brew".to_string());
    }
    None
}

// ── Rust / rustup installation ───────────────────────────────────────────────

/// Install the Rust toolchain via rustup.
///
/// macOS: pipes the official rustup init script through `sh` (the same method
/// documented at <https://rustup.rs>).
///
/// Windows: downloads `rustup-init.exe` from static.rust-lang.org and runs it
/// in non-interactive mode.
pub async fn install_rust(on_progress: &Channel<InstallProgress>) -> Result<(), String> {
    let _ = on_progress.send(InstallProgress::Started {
        service: "Rust".to_string(),
    });

    #[cfg(target_os = "macos")]
    {
        let _ = on_progress.send(InstallProgress::Downloading { percent: 0.0 });

        // Pipe the rustup installer script through sh with -y (non-interactive).
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg("curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y")
            .output()
            .await
            .map_err(|e| format!("Failed to run rustup installer: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let _ = on_progress.send(InstallProgress::Failed { error: stderr.clone() });
            return Err(format!("Rust installation failed: {}", stderr));
        }

        let _ = on_progress.send(InstallProgress::Completed);
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        let _ = on_progress.send(InstallProgress::Downloading { percent: 0.0 });

        let client = reqwest::Client::new();
        let temp_dir = std::env::temp_dir();
        let rustup_path = temp_dir.join("rustup-init.exe");

        let response = client
            .get("https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe")
            .send()
            .await
            .map_err(|e| format!("Failed to download rustup-init: {}", e))?;

        if !response.status().is_success() {
            let msg = format!("rustup-init download failed (HTTP {})", response.status());
            let _ = on_progress.send(InstallProgress::Failed { error: msg.clone() });
            return Err(msg);
        }

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
                let percent = (downloaded as f32 / total as f32) * 80.0;
                let _ = on_progress.send(InstallProgress::Downloading { percent });
            }
        }

        std::fs::write(&rustup_path, &bytes)
            .map_err(|e| format!("Failed to write rustup-init: {}", e))?;

        // Mark the downloaded binary as executable (on Windows this is a no-op;
        // the file is already executable by virtue of being a .exe)
        let _ = &rustup_path; // suppress unused-variable warning if cfg changes

        let _ = on_progress.send(InstallProgress::Installing);

        // Run rustup-init non-interactively; installs the default toolchain.
        let output = tokio::process::Command::new(&rustup_path)
            .args(["-y", "--no-modify-path"])
            .hide_console()
            .output()
            .await
            .map_err(|e| format!("Failed to run rustup-init: {}", e))?;

        // Clean up
        let _ = std::fs::remove_file(&rustup_path);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let _ = on_progress.send(InstallProgress::Failed { error: stderr.clone() });
            return Err(format!("Rust installation failed: {}", stderr));
        }

        let _ = on_progress.send(InstallProgress::Completed);
        Ok(())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = on_progress.send(InstallProgress::Failed {
            error: "Platform not supported".to_string(),
        });
        Err("Platform not supported for automatic Rust installation".to_string())
    }
}
