//! CITADEL Client Kiosk Window Manager
//!
//! Spawns and supervises the full-screen exclusive kiosk browser window
//! pointing directly to the CITADEL offline exam server.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};

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

    let child = Command::new(browser_path)
        .arg(format!("--app={}", target_url))
        .arg("--kiosk")
        .arg("--edge-kiosk-type=fullscreen")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-pinch")
        .arg("--disable-context-menu")
        .arg("--disable-features=TranslateUI,OptimizationHints,MediaRouter")
        .arg(format!("--user-data-dir={}", temp_profile.display()))
        .spawn()?;

    Ok(child)
}
