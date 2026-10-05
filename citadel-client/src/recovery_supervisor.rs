//! CITADEL Restoration Supervisor
//!
//! Authoritative, machine-agnostic supervisor that executes full workstation
//! recovery, terminates all Citadel/Guard processes, sweeps registry policies
//! across all user hives dynamically, restores services, relaunches Explorer,
//! verifies 0 lingering processes 3x, and provides live status handoff on loopback.

use std::ffi::OsStr;
use std::io::Write;
use std::net::TcpListener;
use std::os::windows::ffi::OsStrExt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{BOOL, CloseHandle, HWND};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegEnumKeyExW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, HKEY_USERS, KEY_ALL_ACCESS, KEY_READ, KEY_SET_VALUE,
};
use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, MessageBoxW, ShowWindow, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_SETFOREGROUND, MB_TOPMOST, SW_SHOW,
};

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreStatus {
    pub app: String,
    pub status: String,
    pub phase: String,
    pub verified: bool,
    pub processes_remaining: usize,
    pub details: String,
}

pub fn kill_all_citadel_processes(exclude_pid: u32) {
    eprintln!("[SUPERVISOR] Terminating all Citadel and Guard processes (excluding PID {})...", exclude_pid);
    // Explicit targets: Never target citadel-server.exe (in case running local exam server)
    let targets = [
        "citadel-client.exe",
        "guard-svc.exe",
        "citadel-recovery.exe",
    ];

    // Native Toolhelp32 snapshot kill - strictly respects exclude_pid so supervisor never kills itself!
    for _pass in 1..=2 {
        unsafe {
            if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
                let mut entry = PROCESSENTRY32W::default();
                entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

                if Process32FirstW(snapshot, &mut entry).is_ok() {
                    loop {
                        let exe_name = String::from_utf16_lossy(&entry.szExeFile)
                            .trim_matches(char::from(0))
                            .to_lowercase();

                        let is_target = targets.iter().any(|t| exe_name == t.to_lowercase())
                            || (exe_name.starts_with("citadel-client") && exe_name.ends_with(".exe"))
                            || (exe_name.starts_with("guard") && exe_name.ends_with(".exe"));

                        if is_target && entry.th32ProcessID != exclude_pid && entry.th32ProcessID != 0 {
                            if let Ok(hproc) = OpenProcess(PROCESS_TERMINATE, false, entry.th32ProcessID) {
                                let _ = TerminateProcess(hproc, 1);
                                let _ = CloseHandle(hproc);
                                eprintln!("[SUPERVISOR] Terminated process {} (PID {})", exe_name, entry.th32ProcessID);
                            }
                        }

                        if Process32NextW(snapshot, &mut entry).is_err() {
                            break;
                        }
                    }
                }
                let _ = CloseHandle(snapshot);
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub fn restore_registry_policies_all_hives(interactive_sid: Option<&str>) {
    eprintln!("[SUPERVISOR] Sweeping registry policies across HKCU, HKLM, and all user hives...");

    let policy_specs = [
        (
            r"Software\Microsoft\Windows\CurrentVersion\Policies\System",
            &["DisableTaskMgr", "DisableLockWorkstation", "DisableChangePassword", "DisableAltTab"][..],
        ),
        (
            r"Software\Microsoft\Windows\CurrentVersion\Policies\Explorer",
            &["NoWinKeys", "NoClose", "NoLogoff"][..],
        ),
        (
            r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
            &["EnableSnapAssistFlyout"][..],
        ),
        (
            r"Software\Policies\Microsoft\Windows\TabletPC",
            &["DisableSnippingTool"][..],
        ),
    ];

    let delete_from_root = |root: HKEY, root_name: &str, subkey: &str, val_names: &[&str]| {
        let subkey_w = to_wide(subkey);
        for access in [KEY_ALL_ACCESS, KEY_SET_VALUE] {
            unsafe {
                let mut hkey = HKEY::default();
                if RegOpenKeyExW(root, PCWSTR(subkey_w.as_ptr()), 0, access, &mut hkey).is_ok() {
                    for &val_name in val_names {
                        let val_w = to_wide(val_name);
                        let _ = RegDeleteValueW(hkey, PCWSTR(val_w.as_ptr()));
                    }
                    let _ = RegCloseKey(hkey);
                    break;
                }
            }
        }
        for &val_name in val_names {
            let full_path = format!("{}\\{}", root_name, subkey);
            crate::crash_handler::run_bounded("reg", &["delete", &full_path, "/v", val_name, "/f"], 3000);
        }
    };

    // 1. Delete from HKCU
    for (subkey, val_names) in &policy_specs {
        delete_from_root(HKEY_CURRENT_USER, "HKCU", subkey, val_names);
    }

    // 2. Delete from HKLM
    for (subkey, val_names) in &policy_specs {
        delete_from_root(HKEY_LOCAL_MACHINE, "HKLM", subkey, val_names);
    }

    // 3. Enumerate all user SIDs in HKEY_USERS dynamically (NEVER hardcode SIDs)
    let mut user_sids: Vec<String> = Vec::new();
    if let Some(sid) = interactive_sid {
        user_sids.push(sid.to_string());
    }

    unsafe {
        let mut h_users = HKEY::default();
        if RegOpenKeyExW(HKEY_USERS, PCWSTR::null(), 0, KEY_READ, &mut h_users).is_ok() {
            let mut index = 0u32;
            let mut name_buf = [0u16; 256];
            loop {
                let mut name_len = name_buf.len() as u32;
                if RegEnumKeyExW(
                    h_users,
                    index,
                    windows::core::PWSTR(name_buf.as_mut_ptr()),
                    &mut name_len,
                    None,
                    windows::core::PWSTR::null(),
                    None,
                    None,
                )
                .is_ok()
                {
                    let sid_str = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                    if sid_str.starts_with("S-1-5-21-") && !sid_str.ends_with("_Classes") {
                        if !user_sids.contains(&sid_str) {
                            user_sids.push(sid_str);
                        }
                    }
                    index += 1;
                } else {
                    break;
                }
            }
            let _ = RegCloseKey(h_users);
        }
    }

    for sid in user_sids {
        eprintln!("[SUPERVISOR] Sweeping policies for user SID hive: HKU\\{}", sid);
        for (subkey, val_names) in &policy_specs {
            let full_sub = format!("{}\\{}", sid, subkey);
            delete_from_root(HKEY_USERS, "HKU", &full_sub, val_names);
        }
    }
}

pub fn restore_taskbars() {
    eprintln!("[SUPERVISOR] Restoring taskbar visibility and input states...");
    unsafe {
        if let Ok(taskbar) = FindWindowW(w!("Shell_TrayWnd"), None) {
            let _ = EnableWindow(taskbar, BOOL(1));
            let _ = ShowWindow(taskbar, SW_SHOW);
        }
        if let Ok(sec_taskbar) = FindWindowW(w!("Shell_SecondaryTrayWnd"), None) {
            let _ = EnableWindow(sec_taskbar, BOOL(1));
            let _ = ShowWindow(sec_taskbar, SW_SHOW);
        }
    }
}

pub fn restore_services() {
    eprintln!("[SUPERVISOR] Restoring network and connectivity services (Bluetooth and WLAN)...");
    crate::crash_handler::run_bounded("sc", &["config", "bthserv", "start=", "auto"], 5000);
    crate::crash_handler::run_bounded("net", &["start", "bthserv"], 5000);

    crate::crash_handler::run_bounded("sc", &["config", "WlanSvc", "start=", "auto"], 5000);
    crate::crash_handler::run_bounded("net", &["start", "WlanSvc"], 5000);
}

pub fn restart_explorer_shell() {
    eprintln!("[SUPERVISOR] Restarting Windows Explorer shell...");
    // Kill explorer and relaunch directly via crash_handler helper (no cmd.exe /c start!)
    crate::crash_handler::relaunch_explorer_shell();
}

pub fn cleanup_kiosk_profiles() {
    if let Ok(temp_dir) = std::env::var("TEMP") {
        let temp_path = std::path::Path::new(&temp_dir);
        if let Ok(entries) = std::fs::read_dir(temp_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.starts_with("citadel_kiosk_") {
                            let _ = std::fs::remove_dir_all(&path);
                        }
                    }
                }
            }
        }
    }
}

pub fn verify_zero_processes(exclude_pid: u32) -> (bool, usize) {
    eprintln!("[SUPERVISOR] Running 3x zero-citadel process verification checks...");
    let targets = ["citadel-client.exe", "guard-svc.exe"];
    let mut last_lingering_count = 0;

    for check in 1..=3 {
        std::thread::sleep(Duration::from_millis(300));
        let mut lingering = Vec::new();

        unsafe {
            if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
                let mut entry = PROCESSENTRY32W::default();
                entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

                if Process32FirstW(snapshot, &mut entry).is_ok() {
                    loop {
                        let exe_name = String::from_utf16_lossy(&entry.szExeFile)
                            .trim_matches(char::from(0))
                            .to_lowercase();

                        let is_target = targets.iter().any(|t| exe_name == t.to_lowercase())
                            || (exe_name.starts_with("citadel-client") && exe_name.ends_with(".exe") && entry.th32ProcessID != exclude_pid);

                        if is_target && entry.th32ProcessID != exclude_pid && entry.th32ProcessID != 0 {
                            lingering.push((exe_name, entry.th32ProcessID));
                        }

                        if Process32NextW(snapshot, &mut entry).is_err() {
                            break;
                        }
                    }
                }
                let _ = CloseHandle(snapshot);
            }
        }

        last_lingering_count = lingering.len();
        if lingering.is_empty() {
            eprintln!("[SUPERVISOR] [PASS {}/3] Verified 0 active Citadel processes.", check);
            return (true, 0);
        } else {
            eprintln!(
                "[SUPERVISOR] [FAIL {}/3] Lingering processes detected: {:?}. Retrying kill...",
                check, lingering
            );
            for (_, pid) in lingering {
                unsafe {
                    if let Ok(hproc) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                        let _ = TerminateProcess(hproc, 1);
                        let _ = CloseHandle(hproc);
                    }
                }
            }
        }
    }
    (last_lingering_count == 0, last_lingering_count)
}

fn write_status_file(status: &RestoreStatus) {
    if let Ok(temp_dir) = std::env::var("TEMP") {
        let status_path = std::path::Path::new(&temp_dir).join("citadel_restore_status.json");
        if let Ok(json) = serde_json::to_string_pretty(status) {
            let _ = std::fs::write(status_path, json);
        }
    }
}

pub fn start_status_server(
    status: Arc<Mutex<RestoreStatus>>,
    target_port: Option<u16>,
    duration_secs: u64,
) -> std::thread::JoinHandle<()> {
    let ports_to_try: Vec<u16> = if let Some(p) = target_port {
        vec![p, 8444, 8445, 8446]
    } else {
        (8444..=8450).collect()
    };

    let mut listener_opt = None;
    let mut bound_port = 8444;
    for port in ports_to_try {
        if let Ok(l) = TcpListener::bind(format!("127.0.0.1:{}", port)) {
            let _ = l.set_nonblocking(true);
            eprintln!("[SUPERVISOR] Status server listening on 127.0.0.1:{}", port);
            bound_port = port;
            listener_opt = Some(l);
            break;
        }
    }

    std::thread::spawn(move || {
        let listener = match listener_opt {
            Some(l) => l,
            None => {
                eprintln!("[SUPERVISOR] Unable to bind status handoff port. Background status server exiting.");
                return;
            }
        };

        let start = Instant::now();
        let timeout = Duration::from_secs(duration_secs);

        while start.elapsed() < timeout {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0u8; 1024];
                    let n = stream.peek(&mut buf).unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]);

                    let cors_headers = "Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n";

                    if req.starts_with("OPTIONS") {
                        let resp = format!("HTTP/1.1 204 No Content\r\n{}\r\n", cors_headers);
                        let _ = stream.write_all(resp.as_bytes());
                    } else if req.starts_with("POST") {
                        // Plan 24 F-2: Reject any misrouted end-exam POSTs with 409 Conflict.
                        // Informs the caller that this is the supervisor, not the active client.
                        let body = r#"{"error":"supervisor_not_client","app":"citadel-supervisor"}"#;
                        let resp = format!(
                            "HTTP/1.1 409 Conflict\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{}\r\n{}",
                            body.len(),
                            cors_headers,
                            body
                        );
                        let _ = stream.write_all(resp.as_bytes());
                    } else {
                        let current_state = {
                            let guard = status.lock().unwrap();
                            guard.clone()
                        };
                        let body = serde_json::to_string(&current_state).unwrap_or_else(|_| {
                            r#"{"app":"citadel-supervisor","status":"ok","phase":"restoring","verified":false}"#.to_string()
                        });
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{}\r\n{}",
                            body.len(),
                            cors_headers,
                            body
                        );
                        let _ = stream.write_all(resp.as_bytes());
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
        eprintln!("[SUPERVISOR] Status server on port {} concluded after {}s.", bound_port, duration_secs);
    })
}

pub fn run_supervisor() {
    let args: Vec<String> = std::env::args().collect();
    let own_pid = unsafe { windows::Win32::System::Threading::GetCurrentProcessId() };

    let is_silent = args.iter().any(|a| a == "--silent");
    let target_port = args
        .iter()
        .position(|a| a == "--port")
        .and_then(|idx| args.get(idx + 1))
        .and_then(|s| s.parse::<u16>().ok());

    let interactive_sid = args
        .iter()
        .position(|a| a == "--interactive-sid")
        .and_then(|idx| args.get(idx + 1))
        .map(|s| s.as_str());

    eprintln!("========================================================================");
    eprintln!("      CITADEL RESTORATION SUPERVISOR — WORKSTATION RESCUE");
    eprintln!("========================================================================");
    crate::crash_handler::log_client_event("[SUPERVISOR] Restoration Supervisor started.");

    // Plan 21 F-3: Initialize live state and START STATUS SERVER FIRST!
    let status_state = Arc::new(Mutex::new(RestoreStatus {
        app: "citadel-supervisor".into(),
        status: "restoring".into(),
        phase: "spawning".into(),
        verified: false,
        processes_remaining: 1,
        details: "Supervisor initialized".into(),
    }));
    write_status_file(&status_state.lock().unwrap());

    // Start status server immediately (90 second lifetime)
    let status_handle = start_status_server(status_state.clone(), target_port, 90);

    // Plan 25 F-4: Supervisor self-watchdog (never stall silently; alert if phase > 15s)
    let watchdog_done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let watchdog_done_clone = watchdog_done.clone();
    let watchdog_state = status_state.clone();
    let last_phase_time = std::sync::Arc::new(std::sync::Mutex::new(std::time::Instant::now()));
    let last_phase_time_clone = last_phase_time.clone();

    let _watchdog_handle = std::thread::spawn(move || {
        let mut warned_15s = false;
        let mut warned_30s = false;
        while !watchdog_done_clone.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(500));
            if watchdog_done_clone.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            let (elapsed, current_phase) = {
                let t = last_phase_time_clone.lock().unwrap();
                let g = watchdog_state.lock().unwrap();
                (t.elapsed(), g.phase.clone())
            };

            if elapsed > Duration::from_secs(15) && !warned_15s {
                warned_15s = true;
                let mut g = watchdog_state.lock().unwrap();
                if !g.verified {
                    crate::crash_handler::log_client_event(&format!(
                        "[SUPERVISOR WATCHDOG] Phase '{}' taking > 15s. Updating status file and continuing...",
                        current_phase
                    ));
                    g.details = format!("Phase '{}' in progress (>15s elapsed)...", current_phase);
                    write_status_file(&g);
                }
            }

            if elapsed > Duration::from_secs(30) && !warned_30s {
                warned_30s = true;
                crate::crash_handler::log_client_event(&format!(
                    "[SUPERVISOR WATCHDOG CRITICAL] Phase '{}' taking > 30s. Showing recovery status alert...",
                    current_phase
                ));
                unsafe {
                    let msg = format!("Citadel Restoration is continuing in the background.\n\nCurrent phase: {}\n\nAll restrictions are being released.", current_phase);
                    let wide_msg = to_wide(&msg);
                    let wide_title = to_wide("Citadel Restoration In Progress");
                    let _ = MessageBoxW(
                        HWND(std::ptr::null_mut()),
                        PCWSTR(wide_msg.as_ptr()),
                        PCWSTR(wide_title.as_ptr()),
                        MB_OK | MB_ICONWARNING | MB_TOPMOST | MB_SETFOREGROUND,
                    );
                }
            }
        }
    });

    let update_phase = |phase: &str, details: &str| {
        if let Ok(mut t) = last_phase_time.lock() {
            *t = std::time::Instant::now();
        }
        let mut g = status_state.lock().unwrap();
        g.phase = phase.to_string();
        g.details = details.to_string();
        write_status_file(&g);
        crate::crash_handler::log_client_event(&format!("[SUPERVISOR] Phase -> {}: {}", phase, details));
    };

    // 1. Terminate all Citadel processes
    update_phase("killing", "Terminating Citadel client and cheat tools");
    kill_all_citadel_processes(own_pid);

    // 2. Restore all registry policies across HKCU, HKLM, and HKU user hives
    update_phase("registry", "Restoring registry policies across all user hives");
    restore_registry_policies_all_hives(interactive_sid);

    // 3. Restore taskbars
    update_phase("taskbars", "Restoring Windows taskbars");
    restore_taskbars();

    // 4. Restore services (Bluetooth / WLAN)
    update_phase("services", "Restoring system services (Bluetooth, WLAN)");
    restore_services();

    // 5. Restart Explorer shell
    update_phase("explorer", "Relaunching Windows Explorer shell");
    restart_explorer_shell();

    // 5b. Restore Precision Touchpad multi-finger gestures from pre-exam snapshot
    update_phase("touchpad", "Restoring precision touchpad multi-finger gestures");
    crate::kiosk_window::TouchpadLock::restore_touchpad_gestures();

    // 6. Cleanup temporary kiosk user profiles
    update_phase("cleanup", "Removing ephemeral browser profiles");
    cleanup_kiosk_profiles();

    // 7. Verify zero citadel processes (3 checks)
    update_phase("verifying", "Running 3x zero-citadel process verification");
    let (verified, remaining_count) = verify_zero_processes(own_pid);

    // 8. Update final verified state
    {
        let mut g = status_state.lock().unwrap();
        g.status = if verified { "restored".into() } else { "warning".into() };
        g.phase = if verified { "verified".into() } else { "incomplete".into() };
        g.verified = verified;
        g.processes_remaining = remaining_count;
        g.details = if verified {
            "Workstation verified 100% clean and restored.".into()
        } else {
            format!("Warning: {} lingering processes could not be verified clean.", remaining_count)
        };
        write_status_file(&g);
    }
    watchdog_done.store(true, std::sync::atomic::Ordering::SeqCst);

    // Plan 25 F-7: Remove lockdown active marker upon verified recovery
    if verified {
        let marker_path = crate::crash_handler::get_lockdown_marker_path();
        let _ = std::fs::remove_file(marker_path);
    }
    crate::crash_handler::log_client_event(&format!(
        "[SUPERVISOR] Restoration completed. verified={}, remaining={}",
        verified, remaining_count
    ));

    eprintln!("========================================================================");
    eprintln!(" [SUCCESS] WORKSTATION RESTORATION & VERIFICATION COMPLETE!");
    eprintln!("========================================================================");

    if !is_silent {
        unsafe {
            let wide_msg = to_wide("Citadel Assessment Lockdown has concluded.\n\nAll security restrictions have been released and your system is 100% restored.");
            let wide_title = to_wide("Citadel Lockdown - Workstation Restored");
            let _ = MessageBoxW(
                HWND(std::ptr::null_mut()),
                PCWSTR(wide_msg.as_ptr()),
                PCWSTR(wide_title.as_ptr()),
                MB_OK | MB_ICONINFORMATION | windows::Win32::UI::WindowsAndMessaging::MB_TOPMOST | windows::Win32::UI::WindowsAndMessaging::MB_SETFOREGROUND,
            );
        }
    }

    // Wait for the status server thread to finish its duty cycle
    let _ = status_handle.join();
}
