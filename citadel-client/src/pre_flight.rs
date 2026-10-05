use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::time::Duration;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM, WPARAM};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, EnumDesktopWindows, OpenDesktopW, OpenWindowStationW, SetProcessWindowStation,
    DESKTOP_CONTROL_FLAGS, DESKTOP_ENUMERATE, DESKTOP_READOBJECTS,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, TerminateProcess, PROCESS_TERMINATE,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_NAME_FORMAT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible, MessageBoxW, MESSAGEBOX_STYLE, IDCANCEL, MB_ICONWARNING, MB_OKCANCEL,
    MB_TOPMOST, MB_SETFOREGROUND, PostMessageW, SendMessageTimeoutW, SMTO_ABORTIFHUNG, WM_CLOSE,
};

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct DetectedApplication {
    pub pid: u32,
    pub exe_name: String,
    pub display_name: String,
    pub window_title: Option<String>,
    pub exe_path: Option<String>,
    pub session_id: Option<u32>,
}

pub use crate::policy::PROHIBITED_PROCESSES;

pub fn get_process_full_path(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    unsafe {
        let mut path_buf = [0u16; 1024];
        let mut size = path_buf.len() as u32;
        if let Ok(hproc) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            let res = windows::Win32::System::Threading::QueryFullProcessImageNameW(
                hproc,
                PROCESS_NAME_FORMAT(0),
                windows::core::PWSTR(path_buf.as_mut_ptr()),
                &mut size,
            );
            let _ = CloseHandle(hproc);
            if res.is_ok() && size > 0 {
                return Some(String::from_utf16_lossy(&path_buf[..size as usize]));
            }
        }
    }
    None
}

pub fn get_process_session_id(pid: u32) -> Option<u32> {
    if pid == 0 {
        return None;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn ProcessIdToSessionId(dwProcessId: u32, pSessionId: *mut u32) -> BOOL;
    }
    let mut session_id = 0u32;
    unsafe {
        if ProcessIdToSessionId(pid, &mut session_id).as_bool() {
            Some(session_id)
        } else {
            None
        }
    }
}

/// Gracefully targets and closes File Explorer windows (CabinetWClass, ExploreWClass)
/// without terminating the shell desktop or taskbar processes.
pub fn close_file_explorer_windows(target_pid: u32) {
    struct CloseCtx {
        target_pid: u32,
    }
    unsafe extern "system" fn close_explorer_wnd_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &*(lparam.0 as *const CloseCtx);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        if ctx.target_pid == 0 || pid == ctx.target_pid {
            let mut class_buf = [0u16; 256];
            let class_len = GetClassNameW(hwnd, &mut class_buf);
            if class_len > 0 {
                let class_name = String::from_utf16_lossy(&class_buf[..class_len as usize]);
                if class_name == "CabinetWClass" || class_name == "ExploreWClass" {
                    let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
                    let _ = SendMessageTimeoutW(
                        hwnd,
                        WM_CLOSE,
                        WPARAM(0),
                        LPARAM(0),
                        SMTO_ABORTIFHUNG,
                        400,
                        None,
                    );
                }
            }
        }
        BOOL(1)
    }

    let ctx = CloseCtx { target_pid };
    unsafe {
        let _ = EnumWindows(
            Some(close_explorer_wnd_proc),
            LPARAM(&ctx as *const _ as isize),
        );
    }
}

fn show_dialog(title: &str, message: &str, style: MESSAGEBOX_STYLE) -> windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_RESULT {
    let wide_title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let wide_msg: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        MessageBoxW(
            None,
            PCWSTR(wide_msg.as_ptr()),
            PCWSTR(wide_title.as_ptr()),
            style | MB_TOPMOST | MB_SETFOREGROUND,
        )
    }
}

pub fn get_pid_to_exe_map() -> HashMap<u32, String> {
    let mut map = HashMap::new();
    unsafe {
        if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut entry = PROCESSENTRY32W::default();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let null_pos = entry
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.szExeFile.len());
                    let exe_name = OsString::from_wide(&entry.szExeFile[..null_pos])
                        .to_string_lossy()
                        .to_lowercase();
                    map.insert(entry.th32ProcessID, exe_name);

                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snapshot);
        }
    }
    map
}

struct DesktopEnumContext {
    pid_to_exe: HashMap<u32, String>,
    own_pid: u32,
    detected: HashSet<DetectedApplication>,
}

unsafe extern "system" fn enum_desktop_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut DesktopEnumContext);

    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }

    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return BOOL(1);
    }

    let mut title_buf = vec![0u16; (len + 1) as usize];
    let fetched = GetWindowTextW(hwnd, &mut title_buf);
    if fetched <= 0 {
        return BOOL(1);
    }
    let title = OsString::from_wide(&title_buf[..fetched as usize])
        .to_string_lossy()
        .to_string();

    let mut class_buf = [0u16; 256];
    let class_len = GetClassNameW(hwnd, &mut class_buf);
    let class_name = if class_len > 0 {
        OsString::from_wide(&class_buf[..class_len as usize])
            .to_string_lossy()
            .to_string()
    } else {
        String::new()
    };

    // Ignore Windows Core Shell infrastructure
    if class_name == "Progman"
        || class_name == "WorkerW"
        || class_name == "Shell_TrayWnd"
        || class_name == "Shell_SecondaryTrayWnd"
    {
        return BOOL(1);
    }

    let title_lower = title.to_lowercase();
    if title_lower == "program manager"
        || title_lower == "windows input experience"
        || title_lower == "windows shell experience"
        || title_lower == "status"
    {
        return BOOL(1);
    }

    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));

    if pid == 0 || pid == ctx.own_pid {
        return BOOL(1);
    }

    let exe_name = ctx
        .pid_to_exe
        .get(&pid)
        .cloned()
        .unwrap_or_else(|| "unknown.exe".to_string());
    let exe_lower = exe_name.to_lowercase();

    // Whitelist Citadel's own components and recovery tools
    if exe_lower.contains("citadel")
        || exe_lower.contains("recovery")
        || exe_lower.contains("msedgewebview2")
        || exe_lower.contains("guard-svc")
    {
        return BOOL(1);
    }

    // Special handling for explorer.exe:
    // If the window is a file browsing window (CabinetWClass, ExploreWClass), flag it.
    // Otherwise (desktop/taskbar elements), allow it.
    if exe_lower == "explorer.exe" {
        if class_name == "CabinetWClass" || class_name == "ExploreWClass" {
            let display_title = if title.len() > 40 {
                format!("{}...", &title[..37])
            } else {
                title.clone()
            };
            let full_path = get_process_full_path(pid);
            let session_id = get_process_session_id(pid);
            ctx.detected.insert(DetectedApplication {
                pid,
                exe_name: exe_name.clone(),
                display_name: format!("File Explorer (\"{}\")", display_title),
                window_title: Some(title),
                exe_path: full_path,
                session_id,
            });
        }
        return BOOL(1);
    }

    // Check against unified allowlist (Default = DENY)
    if !crate::policy::is_process_on_allowlist(&exe_name, pid, ctx.own_pid, &HashSet::new()) {
        let display_title = if title.len() > 40 {
            format!("{}...", &title[..37])
        } else {
            title.clone()
        };
        let full_path = get_process_full_path(pid);
        let session_id = get_process_session_id(pid);

        ctx.detected.insert(DetectedApplication {
            pid,
            exe_name: exe_name.clone(),
            display_name: format!("{} (\"{}\")", exe_name, display_title),
            window_title: Some(title),
            exe_path: full_path,
            session_id,
        });
    }

    BOOL(1)
}

/// Comprehensive two-layer scan:
/// 1. Desktop Window Enumerator (EnumDesktopWindows): catches all running GUI apps.
/// 2. Deep Process Snapshot (CreateToolhelp32Snapshot): catches background/minimized apps against the allowlist.
pub fn scan_running_applications(own_pid: u32, is_production: bool) -> Vec<DetectedApplication> {
    let pid_to_exe = get_pid_to_exe_map();
    let mut detected_set = HashSet::new();

    // Layer 1: Enumerate all visible top-level windows on the interactive user desktop
    unsafe {
        let _ = OpenWindowStationW(w!("WinSta0"), false, 0x00020000 | 0x037F)
            .map(|hwinsta| SetProcessWindowStation(hwinsta));

        if let Ok(hdesk) = OpenDesktopW(
            w!("Default"),
            DESKTOP_CONTROL_FLAGS(0),
            false,
            DESKTOP_READOBJECTS.0 | DESKTOP_ENUMERATE.0,
        ) {
            let mut ctx = DesktopEnumContext {
                pid_to_exe: pid_to_exe.clone(),
                own_pid,
                detected: HashSet::new(),
            };

            let _ = EnumDesktopWindows(
                hdesk,
                Some(enum_desktop_windows_proc),
                LPARAM(&mut ctx as *mut _ as isize),
            );
            let _ = CloseDesktop(hdesk);

            for app in ctx.detected {
                detected_set.insert(app);
            }
        }
    }

    // Fail-safe check in Production Mode (Finding B): treat process enumeration failure as error!
    if is_production && pid_to_exe.is_empty() {
        detected_set.insert(DetectedApplication {
            pid: 0,
            exe_name: "PROCESS_ENUMERATION_FAILURE".to_string(),
            display_name: "FATAL: System process snapshot failed. Cannot verify workstation clean.".to_string(),
            window_title: None,
            exe_path: None,
            session_id: None,
        });
    }

    // Layer 2: Deep Toolhelp32 process snapshot against unified allowlist
    let allowlist = crate::policy::Allowlist::citadel_default();
    for (&pid, exe_name) in &pid_to_exe {
        // Skip explorer.exe in Layer 2: File Explorer windows are caught in Layer 1;
        // explorer.exe itself hosts the desktop shell and is permitted by is_windows_system_process.
        if exe_name == "explorer.exe" {
            continue;
        }

        if !allowlist.is_allowed(exe_name, pid, own_pid, &HashSet::new()) {
            let full_path = get_process_full_path(pid);
            let session_id = get_process_session_id(pid);
            detected_set.insert(DetectedApplication {
                pid,
                exe_name: exe_name.clone(),
                display_name: format!("{} (PID: {})", exe_name, pid),
                window_title: None,
                exe_path: full_path,
                session_id,
            });
        }
    }

    let mut result: Vec<DetectedApplication> = detected_set.into_iter().collect();
    result.sort_by(|a, b| a.exe_name.cmp(&b.exe_name));
    result
}

/// Automatically terminates detected applications via taskkill tree kill and Win32 TerminateProcess
pub fn terminate_detected_applications(apps: &[DetectedApplication]) {
    let mut explorer_pids: HashSet<u32> = HashSet::new();
    let mut non_explorer_pids: HashSet<u32> = HashSet::new();
    let mut non_explorer_exes: HashSet<String> = HashSet::new();

    for app in apps {
        if app.exe_name.to_lowercase() == "explorer.exe" {
            explorer_pids.insert(app.pid);
        } else {
            non_explorer_pids.insert(app.pid);
            non_explorer_exes.insert(app.exe_name.clone());
        }
    }

    // A. For File Explorer windows: gracefully close CabinetWClass/ExploreWClass windows without killing explorer.exe shell
    for pid in explorer_pids {
        close_file_explorer_windows(pid);
    }
    close_file_explorer_windows(0);

    // B. For all other unauthorized apps: gather complete process descendants tree
    let seed_pids: Vec<u32> = non_explorer_pids.into_iter().collect();
    let all_target_pids = crate::kiosk_window::find_all_descendants(&seed_pids);

    // 1. Send WM_CLOSE to visible windows first (graceful close)
    struct ClosePidCtx {
        pids: HashSet<u32>,
    }
    unsafe extern "system" fn close_pids_wnd_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &*(lparam.0 as *const ClosePidCtx);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if ctx.pids.contains(&pid) {
            let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
        }
        BOOL(1)
    }
    let close_ctx = ClosePidCtx {
        pids: all_target_pids.iter().cloned().collect(),
    };
    unsafe {
        let _ = EnumWindows(
            Some(close_pids_wnd_proc),
            LPARAM(&close_ctx as *const _ as isize),
        );
    }
    std::thread::sleep(Duration::from_millis(400));

    // 2. Terminate entire process trees by PID (bounded: taskkill /F /T can stall on an uninterruptible process)
    for pid in &all_target_pids {
        crate::crash_handler::run_bounded("taskkill", &["/F", "/T", "/PID", &pid.to_string()], 8000);
    }

    // 3. Terminate matching executable images as belt
    for exe in &non_explorer_exes {
        crate::crash_handler::run_bounded("taskkill", &["/F", "/T", "/IM", exe], 8000);
        eprintln!("[PRE-FLIGHT] Auto-terminated process image: {}", exe);
    }

    // 4. Win32 TerminateProcess fallback for any stubborn process handles
    for pid in &all_target_pids {
        unsafe {
            if let Ok(hproc) = OpenProcess(PROCESS_TERMINATE, false, *pid) {
                let _ = TerminateProcess(hproc, 1);
                let _ = CloseHandle(hproc);
            }
        }
    }
}

/// Runs the complete strict pre-flight scan & clean cycle:
/// 1. Takes pre-exam touchpad snapshot before any modifications.
/// 2. Suppresses Bluetooth service automatically.
/// 3. Performs automated application termination passes (closing all in our hand).
/// 4. Rescans the system.
/// 5. If any applications persist, prompts the user with the exact list.
/// 6. Loops and rescans until ZERO running applications remain. NEVER moves forward while apps are alive.
pub fn enforce_clean_environment(is_production: bool) -> bool {
    let own_pid = unsafe { GetCurrentProcessId() };

    eprintln!("[PRE-FLIGHT] Starting Pre-Launch Environment Enforcement (Production Mode: {})...", is_production);

    // Step 0: Persist pre-exam gesture and system state snapshot (crash safety)
    crate::kiosk_window::TouchpadLock::snapshot_before_exam();

    // Step 1: Suppress Bluetooth hardware & service immediately (bounded -
    // net stop must never stall the pre-flight gate)
    crate::crash_handler::run_bounded("net", &["stop", "bthserv", "/y"], 5000);
    eprintln!("[PRE-FLIGHT] Bluetooth service suppressed for assessment integrity.");

    // Step 2: Automated Termination Phase (Run up to 3 passes to clean all apps in our hand)
    for pass in 1..=3 {
        let detected = scan_running_applications(own_pid, is_production);
        if detected.is_empty() {
            eprintln!("[PRE-FLIGHT] Automated pass {}: All applications verified clean.", pass);
            break;
        }
        eprintln!("[PRE-FLIGHT] Automated pass {}: Auto-terminating {} detected application(s)...", pass, detected.len());
        terminate_detected_applications(&detected);
        std::thread::sleep(Duration::from_millis(700));
    }

    // Step 3: Strict Verification Loop Ã¢â‚¬â€ Prompts candidate and loops until ZERO applications remain
    let mut prompt_count = 0;
    loop {
        // ALWAYS RESCAN FIRST!
        let still_running = scan_running_applications(own_pid, is_production);
        if still_running.is_empty() {
            eprintln!("[PRE-FLIGHT] Workstation verified 100% clean. Zero unauthorized applications running.");
            return true;
        }

        prompt_count += 1;
        let mut app_list_lines: Vec<String> = still_running
            .iter()
            .map(|app| format!("  * {}", app.display_name))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        app_list_lines.sort();

        let list_str = app_list_lines.join("\n");

        let warn_msg = if prompt_count == 1 {
            format!(
                "CITADEL Assessment Security Gatekeeper\n\n\
The following applications are currently open and could not be closed automatically:\n\n\
{}\n\n\
CITADEL policy strictly requires that ALL external applications, browsers,\n\
communication tools, and background utilities must be closed before the\n\
exam environment can open.\n\n\
Please manually save your work and close these applications.\n\n\
Click 'OK' after closing them to re-scan your system.\n\
(Click 'Cancel' to abort and return to your desktop)",
                list_str
            )
        } else {
            format!(
                "CITADEL Security Gatekeeper: ACTION REQUIRED\n\n\
The following application(s) are STILL RUNNING:\n\n\
{}\n\n\
You have NOT closed these applications yet. The assessment CANNOT start\n\
until all listed applications are completely closed.\n\n\
Please open Task Manager or check your taskbar, close them now, and click 'OK' to re-scan.\n\
(Click 'Cancel' to abort)",
                list_str
            )
        };

        let result = show_dialog("CITADEL Security Verification Required", &warn_msg, MB_OKCANCEL | MB_ICONWARNING);

        if result == IDCANCEL {
            eprintln!("[PRE-FLIGHT] Candidate clicked Cancel. Aborting pre-flight safely. Restoring Bluetooth & gestures.");
            crate::crash_handler::run_bounded("net", &["start", "bthserv"], 5000);
            crate::kiosk_window::TouchpadLock::restore_touchpad_gestures();
            return false;
        }

        // When user clicks OK, try one more auto-kill attempt on whatever is left, then sleep and loop to rescan
        terminate_detected_applications(&still_running);
        std::thread::sleep(Duration::from_millis(600));

        // The loop returns to the top and calls scan_running_applications again.
        // If still_running is still not empty, it WILL prompt again!
        // It will NEVER advance until still_running.is_empty() is true!
    }
}

/// Compatibility alias for legacy callers
pub fn scan_prohibited_processes(own_pid: u32, is_production: bool) -> Vec<(String, u32)> {
    scan_running_applications(own_pid, is_production)
        .into_iter()
        .map(|app| (app.exe_name, app.pid))
        .collect()
}

/// Compatibility alias for legacy callers
pub fn terminate_prohibited_processes(processes: &[(String, u32)]) {
    let apps: Vec<DetectedApplication> = processes
        .iter()
        .map(|(exe_name, pid)| DetectedApplication {
            pid: *pid,
            exe_name: exe_name.clone(),
            display_name: format!("{} (PID: {})", exe_name, pid),
            window_title: None,
            exe_path: None,
            session_id: None,
        })
        .collect();
    terminate_detected_applications(&apps);
}