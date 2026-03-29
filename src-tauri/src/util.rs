/// Extension trait that suppresses console windows for spawned processes on Windows.
///
/// On Windows, GUI apps (no console of their own) cause each spawned subprocess to
/// open its own visible console window. Setting the `CREATE_NO_WINDOW` creation flag
/// (`0x08000000`) prevents this — all output goes to the file/pipe the caller provides.
///
/// On macOS and Linux this is a no-op; the method just returns `self` unchanged.
///
/// Usage:
/// ```rust
/// use crate::util::HideConsole;
///
/// let output = std::process::Command::new("python3")
///     .arg("--version")
///     .hide_console()
///     .output()?;
/// ```
pub trait HideConsole {
    fn hide_console(&mut self) -> &mut Self;
}

// ── std::process::Command ─────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
impl HideConsole for std::process::Command {
    fn hide_console(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        self.creation_flags(0x08000000) // CREATE_NO_WINDOW
    }
}

#[cfg(not(target_os = "windows"))]
impl HideConsole for std::process::Command {
    #[inline]
    fn hide_console(&mut self) -> &mut Self {
        self
    }
}

// ── tokio::process::Command ───────────────────────────────────────────────────

#[cfg(target_os = "windows")]
impl HideConsole for tokio::process::Command {
    fn hide_console(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        self.creation_flags(0x08000000) // CREATE_NO_WINDOW
    }
}

#[cfg(not(target_os = "windows"))]
impl HideConsole for tokio::process::Command {
    #[inline]
    fn hide_console(&mut self) -> &mut Self {
        self
    }
}
