use serde::Serialize;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum ServiceStatus {
    NotInstalled,
    Installed,
    Starting,
    Running,
    Stopping,
    Crashed {
        exit_code: Option<i32>,
        restarts: u32,
    },
    /// Using an external instance the user started themselves
    External,
}

pub struct ManagedService {
    pub name: String,
    pub status: ServiceStatus,
    pub process: Option<Child>,
    pub port: u16,
    pub max_restarts: u32,
    pub restart_count: u32,
    /// If true, we detected an externally-running instance and won't manage it
    pub is_external: bool,
}

pub struct ProcessManager {
    pub ollama: RwLock<ManagedService>,
    pub home_assistant: RwLock<ManagedService>,
    app_data_dir: PathBuf,
    log_dir: PathBuf,
}

impl ProcessManager {
    pub fn new(app_data_dir: PathBuf, log_dir: PathBuf) -> Self {
        // Ensure log directory exists
        let _ = std::fs::create_dir_all(&log_dir);

        Self {
            ollama: RwLock::new(ManagedService {
                name: "Ollama".to_string(),
                status: ServiceStatus::NotInstalled,
                process: None,
                port: 11434,
                max_restarts: 3,
                restart_count: 0,
                is_external: false,
            }),
            home_assistant: RwLock::new(ManagedService {
                name: "Home Assistant".to_string(),
                status: ServiceStatus::NotInstalled,
                process: None,
                port: 8123,
                max_restarts: 3,
                restart_count: 0,
                is_external: false,
            }),
            app_data_dir,
            log_dir,
        }
    }

    /// Check if a port is already in use by trying a health check
    pub async fn is_port_in_use(&self, port: u16) -> bool {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap_or_default();

        client
            .get(format!("http://localhost:{}", port))
            .send()
            .await
            .is_ok()
    }

    /// Check if Ollama is healthy on its port
    pub async fn check_ollama_health(&self) -> bool {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap_or_default();

        client
            .get("http://localhost:11434/")
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Check if Home Assistant is healthy on its port
    pub async fn check_ha_health(&self) -> bool {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap_or_default();

        // HA returns 401 when running but needs auth — that still means it's alive
        match client.get("http://localhost:8123/api/").send().await {
            Ok(r) => {
                let status = r.status().as_u16();
                status == 200 || status == 401 || status == 403
            }
            Err(_) => false,
        }
    }

    /// Detect if Ollama is already running externally
    pub async fn detect_external_ollama(&self) -> bool {
        self.check_ollama_health().await
    }

    /// Detect if HA is already running externally
    pub async fn detect_external_ha(&self) -> bool {
        self.check_ha_health().await
    }

    /// Find the Ollama binary path
    pub fn find_ollama_binary(&self) -> Option<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            let candidates = [
                "/usr/local/bin/ollama",
                "/opt/homebrew/bin/ollama",
            ];
            for path in &candidates {
                let p = PathBuf::from(path);
                if p.exists() {
                    return Some(p);
                }
            }
            // Check PATH
            if let Ok(output) = Command::new("which").arg("ollama").output() {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() {
                        return Some(PathBuf::from(path));
                    }
                }
            }
            // Check user-local install
            if let Some(home) = dirs_next_home() {
                let p = home.join(".ollama/ollama");
                if p.exists() {
                    return Some(p);
                }
            }
            None
        }

        #[cfg(target_os = "windows")]
        {
            // Check PATH
            if let Ok(output) = Command::new("where").arg("ollama").output() {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if !path.is_empty() {
                        return Some(PathBuf::from(path));
                    }
                }
            }
            // Common install locations
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                let p = PathBuf::from(&local).join("Programs\\Ollama\\ollama.exe");
                if p.exists() {
                    return Some(p);
                }
            }
            if let Ok(pf) = std::env::var("PROGRAMFILES") {
                let p = PathBuf::from(&pf).join("Ollama\\ollama.exe");
                if p.exists() {
                    return Some(p);
                }
            }
            None
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            None
        }
    }

    /// Find the Home Assistant (hass) binary in our managed venv
    pub fn find_ha_binary(&self) -> Option<PathBuf> {
        let venv = self.app_data_dir.join("ha-venv");

        #[cfg(target_os = "macos")]
        let hass = venv.join("bin/hass");

        #[cfg(target_os = "windows")]
        let hass = venv.join("Scripts\\hass.exe");

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let hass = venv.join("bin/hass");

        if hass.exists() {
            Some(hass)
        } else {
            None
        }
    }

    /// Start Ollama as a child process
    pub async fn start_ollama(&self) -> Result<(), String> {
        {
            let svc = self.ollama.read().await;
            if svc.is_external {
                return Ok(());
            }
            if svc.status == ServiceStatus::Running {
                return Ok(());
            }
        }

        let binary = self
            .find_ollama_binary()
            .ok_or_else(|| "Ollama binary not found".to_string())?;

        {
            let mut svc = self.ollama.write().await;
            svc.status = ServiceStatus::Starting;
        }

        let log_path = self.log_dir.join("ollama.log");
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| format!("Failed to open log file: {}", e))?;

        let log_file_err = log_file
            .try_clone()
            .map_err(|e| format!("Failed to clone log handle: {}", e))?;

        let mut cmd = Command::new(binary);
        cmd.arg("serve")
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_file_err));

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start Ollama: {}", e))?;

        {
            let mut svc = self.ollama.write().await;
            svc.process = Some(child);
        }

        // Wait for Ollama to become healthy
        let healthy = self.wait_for_health("ollama", 30).await;
        {
            let mut svc = self.ollama.write().await;
            if healthy {
                svc.status = ServiceStatus::Running;
            } else {
                svc.status = ServiceStatus::Crashed {
                    exit_code: None,
                    restarts: svc.restart_count,
                };
                return Err("Ollama failed to start within 30 seconds".to_string());
            }
        }

        Ok(())
    }

    /// Start Home Assistant as a child process
    pub async fn start_ha(&self) -> Result<(), String> {
        {
            let svc = self.home_assistant.read().await;
            if svc.is_external {
                return Ok(());
            }
            if svc.status == ServiceStatus::Running {
                return Ok(());
            }
        }

        let binary = self
            .find_ha_binary()
            .ok_or_else(|| "Home Assistant not installed in managed venv".to_string())?;

        {
            let mut svc = self.home_assistant.write().await;
            svc.status = ServiceStatus::Starting;
        }

        let ha_config_dir = self.app_data_dir.join("ha-config");
        let _ = std::fs::create_dir_all(&ha_config_dir);

        let log_path = self.log_dir.join("homeassistant.log");
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| format!("Failed to open log file: {}", e))?;

        let log_file_err = log_file
            .try_clone()
            .map_err(|e| format!("Failed to clone log handle: {}", e))?;

        let mut cmd = Command::new(binary);
        cmd.arg("--config")
            .arg(&ha_config_dir)
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_file_err));

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start Home Assistant: {}", e))?;

        {
            let mut svc = self.home_assistant.write().await;
            svc.process = Some(child);
        }

        // HA takes longer to start
        let healthy = self.wait_for_health("home_assistant", 60).await;
        {
            let mut svc = self.home_assistant.write().await;
            if healthy {
                svc.status = ServiceStatus::Running;
            } else {
                svc.status = ServiceStatus::Crashed {
                    exit_code: None,
                    restarts: svc.restart_count,
                };
                return Err("Home Assistant failed to start within 60 seconds".to_string());
            }
        }

        Ok(())
    }

    /// Wait for a service to pass health checks
    async fn wait_for_health(&self, service: &str, timeout_secs: u64) -> bool {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);

        while start.elapsed() < timeout {
            let healthy = match service {
                "ollama" => self.check_ollama_health().await,
                "home_assistant" => self.check_ha_health().await,
                _ => false,
            };
            if healthy {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        false
    }

    /// Stop a service gracefully
    pub async fn stop_service(&self, service: &str) -> Result<(), String> {
        let svc_lock = match service {
            "ollama" => &self.ollama,
            "home_assistant" => &self.home_assistant,
            _ => return Err(format!("Unknown service: {}", service)),
        };

        let mut svc = svc_lock.write().await;
        if svc.is_external {
            return Ok(());
        }

        svc.status = ServiceStatus::Stopping;

        if let Some(ref mut child) = svc.process {
            // Try graceful kill first
            #[cfg(unix)]
            {
                unsafe {
                    libc::kill(child.id() as i32, libc::SIGTERM);
                }
            }

            #[cfg(windows)]
            {
                let _ = child.kill();
            }

            // Wait up to 5s for exit
            let start = std::time::Instant::now();
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => {
                        if start.elapsed() > Duration::from_secs(5) {
                            // Force kill
                            let _ = child.kill();
                            let _ = child.wait();
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                    Err(_) => break,
                }
            }
        }

        svc.process = None;
        svc.status = ServiceStatus::Installed;

        Ok(())
    }

    /// Restart a specific service
    pub async fn restart_service(&self, service: &str) -> Result<(), String> {
        self.stop_service(service).await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        match service {
            "ollama" => self.start_ollama().await,
            "home_assistant" => self.start_ha().await,
            _ => Err(format!("Unknown service: {}", service)),
        }
    }

    /// Gracefully shut down all managed services
    pub async fn shutdown_all(&self) {
        let _ = self.stop_service("ollama").await;
        let _ = self.stop_service("home_assistant").await;
    }

    /// Get the current status of a service
    pub async fn get_status(&self, service: &str) -> ServiceStatus {
        match service {
            "ollama" => self.ollama.read().await.status.clone(),
            "home_assistant" => self.home_assistant.read().await.status.clone(),
            _ => ServiceStatus::NotInstalled,
        }
    }

    /// Start the background health monitoring loop (call once at startup)
    pub fn start_monitoring(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;

                // Check Ollama
                {
                    let svc = self.ollama.read().await;
                    if !svc.is_external && svc.status == ServiceStatus::Running {
                        drop(svc);
                        if !self.check_ollama_health().await {
                            let mut svc = self.ollama.write().await;
                            if svc.restart_count < svc.max_restarts {
                                svc.restart_count += 1;
                                let count = svc.restart_count;
                                drop(svc);
                                eprintln!("Ollama health check failed, restarting (attempt {})", count);
                                let _ = self.restart_service("ollama").await;
                            } else {
                                svc.status = ServiceStatus::Crashed {
                                    exit_code: None,
                                    restarts: svc.restart_count,
                                };
                            }
                        }
                    }
                }

                // Check Home Assistant
                {
                    let svc = self.home_assistant.read().await;
                    if !svc.is_external && svc.status == ServiceStatus::Running {
                        drop(svc);
                        if !self.check_ha_health().await {
                            let mut svc = self.home_assistant.write().await;
                            if svc.restart_count < svc.max_restarts {
                                svc.restart_count += 1;
                                let count = svc.restart_count;
                                drop(svc);
                                eprintln!("HA health check failed, restarting (attempt {})", count);
                                let _ = self.restart_service("home_assistant").await;
                            } else {
                                svc.status = ServiceStatus::Crashed {
                                    exit_code: None,
                                    restarts: svc.restart_count,
                                };
                            }
                        }
                    }
                }
            }
        });
    }
}

/// Get the user's home directory
fn dirs_next_home() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        std::env::var("HOME").ok().map(PathBuf::from)
    }

    #[cfg(target_os = "windows")]
    {
        std::env::var("USERPROFILE").ok().map(PathBuf::from)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}
