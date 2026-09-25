//! CITADEL Client Kiosk Window Manager
//!
//! Spawns and supervises the full-screen exclusive kiosk browser window
//! and hides the Windows OS taskbar and Start button.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use windows::core::w;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, ShowWindow, SW_HIDE, SW_SHOW};

/// RAII Guard that hides the Windows Shell Taskbar while active,
/// and restores it automatically when dropped.
pub struct TaskbarLock;

impl TaskbarLock {
    pub fn acquire() -> Self {
        unsafe {
            if let Ok(taskbar) = FindWindowW(w!("Shell_TrayWnd"), None) {
                let _ = ShowWindow(taskbar, SW_HIDE);
            }
            if let Ok(sec_taskbar) = FindWindowW(w!("Shell_SecondaryTrayWnd"), None) {
                let _ = ShowWindow(sec_taskbar, SW_HIDE);
            }
        }
        TaskbarLock
    }
}

impl Drop for TaskbarLock {
    fn drop(&mut self) {
        unsafe {
            if let Ok(taskbar) = FindWindowW(w!("Shell_TrayWnd"), None) {
                let _ = ShowWindow(taskbar, SW_SHOW);
            }
            if let Ok(sec_taskbar) = FindWindowW(w!("Shell_SecondaryTrayWnd"), None) {
                let _ = ShowWindow(sec_taskbar, SW_SHOW);
            }
        }
    }
}

pub fn find_browser_executable() -> Option<PathBuf> {
    let candidate_paths = [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    ];

    for path in candidate_paths {
        let p = Path::new(path);
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }

    None
}

pub fn launch_kiosk(target_url: &str) -> std::io::Result<Child> {
    let browser_path = find_browser_executable()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No supported browser (Edge/Chrome) found on system"))?;

    // Create an isolated temp profile directory for the kiosk session
    let temp_profile = std::env::temp_dir().join(format!("citadel_kiosk_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_profile);

    println!("[CITADEL CLIENT] Spawning exclusive kiosk window via: {:?}", browser_path);
    println!("[CITADEL CLIENT] Connecting to exam endpoint: {}", target_url);

    // CRITICAL: --user-data-dir and --new-window must come FIRST
    // so Chromium creates an isolated process and never delegates to existing browser instances!
    let child = Command::new(browser_path)
        .arg(format!("--user-data-dir={}", temp_profile.display()))
        .arg("--new-window")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--kiosk")
        .arg("--edge-kiosk-type=fullscreen")
        .arg("--disable-pinch")
        .arg("--disable-context-menu")
        .arg("--user-agent=CITADEL-Lockdown-Client/1.0 (Windows NT 10.0; Win64; x64; CitadelSecurityCore)")
        .arg("--disable-features=TranslateUI,OptimizationHints,MediaRouter")
        .arg(format!("--app={}", target_url))
        .spawn()?;

    Ok(child)
}
