//! CITADEL Client Module: Failsafe Crash Recovery & Panic Handler
//!
//! Ensures that if the client process encounters an unhandled panic, OS termination
//! signal, or unexpected crash:
//! 1. Registry escape locks (DisableTaskMgr, NoWinKeys, etc.) are erased/restored.
//! 2. Display plane is switched back to the default desktop.
//! 3. Windows Explorer shell (explorer.exe) is restarted.
//! 4. Bluetooth and WLAN services are restored.

use std::ffi::OsStr;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::process::Command;
use std::os::windows::process::CommandExt;
use std::time::Duration;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{BOOL, CloseHandle};
use windows::Win32::System::Console::SetConsoleCtrlHandler;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_WRITE,
};
use windows::Win32::System::StationsAndDesktops::{
    OpenDesktopW, SetThreadDesktop, SwitchDesktop, DESKTOP_CONTROL_FLAGS,
};

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

/// Appends a diagnostic line to the client log files. Library-side breadcrumb
/// logging so startup hangs and failures are diagnosable from
/// %TEMP%\citadel_client.log even in `windows_subsystem` builds where stderr
/// is invisible.
pub fn log_client_event(msg: &str) {
    let mut paths = vec![format!(
        r"{}\citadel_client.log",
        std::env::temp_dir().display()
    )];
    if let Ok(profile) = std::env::var("USERPROFILE") {
        paths.push(format!(r"{}\citadel_client.log", profile));
    }
    for p in &paths {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            let now = format!("{:?}", std::time::SystemTime::now());
            let _ = writeln!(f, "[{}] {}", now, msg);
        }
    }
}

/// Runs a service-control command with a bounded wait. `net stop` / `net start`
/// can block for tens of seconds when a service ignores its control signal;
/// guard initialization and crash restoration must never stall on them, so the
/// wait is capped and the command is simply left running if it exceeds the cap.
pub fn run_bounded(program: &str, args: &[&str], timeout_ms: u64) {
    let child = Command::new(program)
        .args(args)
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn();
    if let Ok(mut child) = child {
        let mut waited_ms = 0u64;
        while waited_ms < timeout_ms {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => {
                    std::thread::sleep(Duration::from_millis(100));
                    waited_ms += 100;
                }
                Err(_) => return,
            }
        }
        eprintln!(
            "[CITADEL CLIENT] Warning: '{}' still running after {}ms; continuing without waiting.",
            program, timeout_ms
        );
    }
}

/// Restores the Windows Explorer shell exactly once: relaunches explorer.exe
/// only when no instance is running. Spawning explorer.exe while the shell is
/// already alive opens a new File Explorer window instead of restoring the
/// desktop, so unconditional spawns stack duplicate windows across repeated
/// crash-restore cycles.

pub fn get_lockdown_marker_path() -> std::path::PathBuf {
    if let Ok(progdata) = std::env::var("ProgramData") {
        let p = std::path::Path::new(&progdata).join("Citadel").join("state");
        let _ = std::fs::create_dir_all(&p);
        p.join("lockdown_active.json")
    } else {
        std::env::temp_dir().join("citadel_lockdown_active.json")
    }
}

pub fn is_other_citadel_client_running(own_pid: u32) -> bool {
    unsafe {
        if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut entry = PROCESSENTRY32W::default();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let exe_name = String::from_utf16_lossy(&entry.szExeFile)
                        .trim_matches(char::from(0))
                        .to_lowercase();
                    if (exe_name == "citadel-client.exe" || (exe_name.starts_with("citadel-client") && exe_name.ends_with(".exe")))
                        && entry.th32ProcessID != own_pid
                        && entry.th32ProcessID != 0
                    {
                        let _ = CloseHandle(snapshot);
                        return true;
                    }
                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snapshot);
        }
    }
    false
}

pub fn relaunch_explorer_shell() {
    let mut running = false;
    unsafe {
        if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut entry = PROCESSENTRY32W::default();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let exe_name = String::from_utf16_lossy(&entry.szExeFile)
                        .trim_matches(char::from(0))
                        .to_lowercase();
                    if exe_name == "explorer.exe" {
                        running = true;
                        break;
                    }
                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snapshot);
        }
    }

    if !running {
        let _ = Command::new("explorer.exe").spawn();
    }
}

pub fn emergency_restore_system() {
    eprintln!("[CITADEL EMERGENCY] Initiating failsafe system restoration...");

    // Write crash recovery marker for supervisor / reboot audit
    let own_pid = unsafe { windows::Win32::System::Threading::GetCurrentProcessId() };
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let marker = serde_json::json!({
        "pid": own_pid,
        "timestamp": timestamp,
        "event": "emergency_restore_system"
    });
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".to_string());
    let marker_dir = std::path::Path::new(&program_data).join("Citadel");
    let _ = std::fs::create_dir_all(&marker_dir);
    let marker_path = marker_dir.join("crash_recovery.json");
    let _ = std::fs::write(marker_path, marker.to_string());

    // 1. Restore/delete registry locks
    let keys = [
        (
            r"Software\Microsoft\Windows\CurrentVersion\Policies\System",
            "DisableTaskMgr",
        ),
        (
            r"Software\Microsoft\Windows\CurrentVersion\Policies\System",
            "DisableLockWorkstation",
        ),
        (
            r"Software\Microsoft\Windows\CurrentVersion\Policies\System",
            "DisableChangePassword",
        ),
        (
            r"Software\Microsoft\Windows\CurrentVersion\Policies\Explorer",
            "NoWinKeys",
        ),
        (
            r"Software\Microsoft\Windows\CurrentVersion\Policies\Explorer",
            "NoClose",
        ),
        (
            r"Software\Microsoft\Windows\CurrentVersion\Policies\Explorer",
            "NoLogoff",
        ),
        (
            r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
            "EnableSnapAssistFlyout",
        ),
        (
            r"Software\Microsoft\Windows\CurrentVersion\Policies\System",
            "DisableAltTab",
        ),
        (
            r"Software\Policies\Microsoft\Windows\TabletPC",
            "DisableSnippingTool",
        ),
    ];

    for (subkey, val_name) in keys {
        let subkey_w = to_wide(subkey);
        let val_w = to_wide(val_name);
        unsafe {
            let mut hkey = HKEY::default();
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey_w.as_ptr()),
                0,
                KEY_WRITE,
                &mut hkey,
            ).is_ok() {
                let _ = RegDeleteValueW(hkey, PCWSTR(val_w.as_ptr()));
                let _ = RegCloseKey(hkey);
            }
        }
    }

    // 2. Switch back to Default desktop if possible
    unsafe {
        let default_name = to_wide("Default");
        if let Ok(default_desk) = OpenDesktopW(
            PCWSTR(default_name.as_ptr()),
            DESKTOP_CONTROL_FLAGS(0),
            false,
            0x0100, // DESKTOP_SWITCHDESKTOP
        ) {
            let _ = SwitchDesktop(default_desk);
            let _ = SetThreadDesktop(default_desk);
        }
    }

    // 3. Restart explorer.exe (idempotent: only when no shell instance is alive)
    relaunch_explorer_shell();

    // 4. Restore Bluetooth and WLAN (bounded waits â€” never stall crash recovery)
    run_bounded("sc", &["config", "bthserv", "start=", "auto"], 5000);
    run_bounded("net", &["start", "bthserv"], 5000);
    run_bounded("sc", &["config", "WlanSvc", "start=", "auto"], 5000);
    run_bounded("net", &["start", "WlanSvc"], 5000);

    // 5. Restore Precision Touchpad multi-finger gestures from pre-exam snapshot
    crate::kiosk_window::TouchpadLock::restore_touchpad_gestures();

    let marker_path = get_lockdown_marker_path();
    let _ = std::fs::remove_file(marker_path);
    eprintln!("[CITADEL EMERGENCY] Failsafe restoration executed.");
}

unsafe extern "system" fn console_ctrl_handler(_ctrl_type: u32) -> BOOL {
    emergency_restore_system();
    BOOL(0) // Return FALSE to allow default handler to terminate process
}

pub fn install_crash_safety() {
    // 1. Install Console Control Handler (for close/shutdown events)
    unsafe {
        let _ = SetConsoleCtrlHandler(Some(console_ctrl_handler), true);
    }

    // 2. Install Rust Panic Hook
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        emergency_restore_system();
        default_hook(panic_info);
    }));
}
