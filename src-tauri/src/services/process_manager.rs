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
    pub status: ServiceStatus,
    pub process: Option<Child>,
    pub max_restarts: u32,
    pub restart_count: u32,
    /// If true, we detected an externally-running instance and won't manage it
    pub is_external: bool,
}

pub struct ProcessManager {
    pub ollama: RwLock<ManagedService>,
    pub home_assistant: RwLock<ManagedService>,
    app_data_dir: PathBuf,
    ha_venv_dir: PathBuf,
    log_dir: PathBuf,
}

impl ProcessManager {
    pub fn new(app_data_dir: PathBuf, log_dir: PathBuf) -> Self {
        // Ensure log directory exists
        let _ = std::fs::create_dir_all(&log_dir);

        let ha_venv_dir = super::installer::ha_venv_dir();

        Self {
            ollama: RwLock::new(ManagedService {
                status: ServiceStatus::NotInstalled,
                process: None,
                max_restarts: 3,
                restart_count: 0,
                is_external: false,
            }),
            home_assistant: RwLock::new(ManagedService {
                status: ServiceStatus::NotInstalled,
                process: None,
                max_restarts: 3,
                restart_count: 0,
                is_external: false,
            }),
            app_data_dir,
            ha_venv_dir,
            log_dir,
        }
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

}

/// Kill any process currently listening on port 8123 (stale HA instances).
/// Uses platform-specific tools to find and SIGKILL the process.
pub fn kill_process_on_port_8123() {
    #[cfg(unix)]
    {
        // lsof -ti :8123 returns the PID(s) using the port
        if let Ok(output) = Command::new("lsof")
            .args(["-ti", ":8123"])
            .output()
        {
            let pids = String::from_utf8_lossy(&output.stdout);
            for pid_str in pids.split_whitespace() {
                if let Ok(pid) = pid_str.trim().parse::<i32>() {
                    println!("[HA] killing stale process on :8123 (pid: {})", pid);
                    unsafe { libc::kill(pid, libc::SIGKILL); }
                }
            }
            // Give the OS a moment to reclaim the port
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    #[cfg(windows)]
    {
        // netstat -ano | findstr :8123, then taskkill /F /PID <pid>
        if let Ok(output) = Command::new("netstat")
            .args(["-ano"])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if line.contains(":8123") && line.contains("LISTENING") {
                    let pid = line.split_whitespace().last().unwrap_or("").trim();
                    if !pid.is_empty() {
                        println!("[HA] killing stale process on :8123 (pid: {})", pid);
                        let _ = Command::new("taskkill")
                            .args(["/F", "/PID", pid])
                            .output();
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }
}

impl ProcessManager {
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

    /// Find the Home Assistant (hass) binary — checks managed venv first, then system PATH
    pub fn find_ha_binary(&self) -> Option<PathBuf> {
        // 1. Check managed venv (space-free path — see installer::ha_venv_dir() for rationale)
        let venv = &self.ha_venv_dir;

        #[cfg(target_os = "macos")]
        let hass = venv.join("bin/hass");

        #[cfg(target_os = "windows")]
        let hass = venv.join("Scripts\\hass.exe");

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let hass = venv.join("bin/hass");

        if hass.exists() {
            return Some(hass);
        }

        // 2. Check system PATH
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = Command::new("which").arg("hass").output() {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() {
                        return Some(PathBuf::from(path));
                    }
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(output) = Command::new("where").arg("hass").output() {
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
        }

        None
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
            if svc.status == ServiceStatus::Running {
                println!("[HA] already running");
                return Ok(());
            }
        }

        println!("[HA] starting Home Assistant...");

        let binary = self
            .find_ha_binary()
            .ok_or_else(|| "Home Assistant not installed in managed venv".to_string())?;

        println!("[HA] binary:  {}", binary.display());

        {
            let mut svc = self.home_assistant.write().await;
            svc.status = ServiceStatus::Starting;
        }

        let ha_config_dir = self.app_data_dir.join("ha-config");
        let _ = std::fs::create_dir_all(&ha_config_dir);
        println!("[HA] config:  {}", ha_config_dir.display());

        let log_path = self.log_dir.join("homeassistant.log");
        println!("[HA] log:     {}", log_path.display());

        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| format!("Failed to open log file: {}", e))?;

        let log_file_err = log_file
            .try_clone()
            .map_err(|e| format!("Failed to clone log handle: {}", e))?;

        let mut cmd = Command::new(&binary);

        // Build a clean PATH without entries containing spaces.
        // HA's internal pip/uv chokes on PATH entries with spaces (e.g., macOS
        // "Application Support") — it misparses them as package requirements.
        let clean_path = {
            let venv_bin = binary.parent().unwrap_or(std::path::Path::new(""));
            let original_path = std::env::var("PATH").unwrap_or_default();
            let mut parts: Vec<&str> = original_path
                .split(':')
                .filter(|p| !p.contains(' '))
                .collect();
            let venv_str = venv_bin.to_str().unwrap_or("");
            if !venv_str.is_empty() && !parts.contains(&venv_str) {
                parts.insert(0, venv_str);
            }
            parts.join(":")
        };

        cmd.arg("--config")
            .arg(&ha_config_dir)
            .env("PATH", &clean_path)
            .env_remove("PYTHONPATH")
            .env_remove("PYTHONHOME")
            .env_remove("PIP_REQUIRE_VIRTUALENV")
            .env_remove("VIRTUAL_ENV")
            .env_remove("UV_CONSTRAINT")
            .env_remove("PIP_CONSTRAINT")
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

        println!("[HA] spawned (pid: {})", child.id());

        {
            let mut svc = self.home_assistant.write().await;
            svc.process = Some(child);
        }

        // HA takes longer to start — first run can take 2+ minutes
        // (database creation, onboarding setup, dependency installs)
        const TIMEOUT_SECS: u64 = 180;
        println!("[HA] waiting for health check on :8123 (timeout: {TIMEOUT_SECS}s)...");

        let healthy = self.wait_for_health("home_assistant", TIMEOUT_SECS).await;
        {
            let mut svc = self.home_assistant.write().await;
            if healthy {
                svc.status = ServiceStatus::Running;
                println!("[HA] ✓ running");
            } else {
                svc.status = ServiceStatus::Crashed {
                    exit_code: None,
                    restarts: svc.restart_count,
                };
                eprintln!("[HA] ✗ failed to become healthy within {TIMEOUT_SECS}s");
                eprintln!("[HA]   check log: {}", self.log_dir.join("homeassistant.log").display());
                return Err(format!("Home Assistant failed to start within {TIMEOUT_SECS} seconds"));
            }
        }

        Ok(())
    }

    /// Wait for a service to pass health checks
    async fn wait_for_health(&self, service: &str, timeout_secs: u64) -> bool {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);
        let mut last_log_secs: u64 = 0;

        while start.elapsed() < timeout {
            let healthy = match service {
                "ollama" => self.check_ollama_health().await,
                "home_assistant" => self.check_ha_health().await,
                _ => false,
            };
            if healthy {
                return true;
            }

            let elapsed = start.elapsed().as_secs();
            if elapsed >= last_log_secs + 10 {
                let label = match service {
                    "home_assistant" => "HA",
                    "ollama" => "Ollama",
                    other => other,
                };
                println!("[{label}] still starting... ({elapsed}s elapsed)");
                last_log_secs = elapsed;
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
