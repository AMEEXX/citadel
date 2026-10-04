use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::time::Duration;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, EnumDesktopWindows, OpenDesktopW, OpenWindowStationW, SetProcessWindowStation,
    DESKTOP_CONTROL_FLAGS, DESKTOP_ENUMERATE, DESKTOP_READOBJECTS,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, TerminateProcess, PROCESS_TERMINATE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    MessageBoxW, MESSAGEBOX_STYLE, IDCANCEL, MB_ICONWARNING, MB_OKCANCEL,
};

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct DetectedApplication {
    pub pid: u32,
    pub exe_name: String,
    pub display_name: String,
}

pub use crate::policy::PROHIBITED_PROCESSES;

fn show_dialog(title: &str, message: &str, style: MESSAGEBOX_STYLE) -> windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_RESULT {
    let wide_title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let wide_msg: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        MessageBoxW(
            None,
            PCWSTR(wide_msg.as_ptr()),
            PCWSTR(wide_title.as_ptr()),
            style,
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
    policy: crate::policy::LockdownPolicy,
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
    {
        return BOOL(1);
    }

    // Whitelist standard Windows OS system utilities
    let is_system_os = exe_lower == "explorer.exe"
        || exe_lower == "dwm.exe"
        || exe_lower == "sihost.exe"
        || exe_lower == "taskhostw.exe"
        || exe_lower == "runtimebroker.exe"
        || exe_lower == "textinputhost.exe"
        || exe_lower == "searchhost.exe"
        || exe_lower == "startmenuexperiencehost.exe"
        || exe_lower == "csrss.exe"
        || exe_lower == "smss.exe"
        || exe_lower == "services.exe"
        || exe_lower == "lsass.exe"
        || exe_lower == "winlogon.exe"
        || exe_lower == "fontdrvhost.exe"
        || exe_lower == "ctfmon.exe"
        || exe_lower == "conhost.exe"
        || exe_lower == "audiodg.exe"
        || exe_lower == "spoolsv.exe"
        || exe_lower == "smartscreen.exe"
        || exe_lower.contains("quick heal")
        || exe_lower.contains("mcafee")
        || exe_lower.contains("defender");

    if is_system_os {
        return BOOL(1);
    }

    // Check against unified policy
    if !ctx.policy.is_process_allowed(&exe_name, Some(&title), pid, ctx.own_pid, &HashSet::new()) {
        let display_title = if title.len() > 40 {
            format!("{}...", &title[..37])
        } else {
            title
        };

        ctx.detected.insert(DetectedApplication {
            pid,
            exe_name: exe_name.clone(),
            display_name: format!("{} (\"{}\")", exe_name, display_title),
        });
    }

    BOOL(1)
}

/// Comprehensive two-layer scan:
/// 1. Desktop Window Enumerator (`EnumDesktopWindows`): catches all running GUI apps.
/// 2. Deep Process Snapshot (`CreateToolhelp32Snapshot`): catches background/minimized prohibited apps.
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
                policy: crate::policy::LockdownPolicy::new(is_production),
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
        });
    }

    // Layer 2: Deep Toolhelp32 process snapshot against unified policy
    let policy = crate::policy::LockdownPolicy::new(is_production);
    for (&pid, exe_name) in &pid_to_exe {
        if !policy.is_process_allowed(exe_name, None, pid, own_pid, &HashSet::new()) {
            detected_set.insert(DetectedApplication {
                pid,
                exe_name: exe_name.clone(),
                display_name: format!("{} (PID: {})", exe_name, pid),
            });
        }
    }

    let mut result: Vec<DetectedApplication> = detected_set.into_iter().collect();
    result.sort_by(|a, b| a.exe_name.cmp(&b.exe_name));
    result
}

/// Automatically terminates detected applications via taskkill tree kill and Win32 TerminateProcess
pub fn terminate_detected_applications(apps: &[DetectedApplication]) {
    let mut unique_pids: HashSet<u32> = HashSet::new();
    let mut unique_exes: HashSet<String> = HashSet::new();

    for app in apps {
        unique_pids.insert(app.pid);
        unique_exes.insert(app.exe_name.clone());
    }

    // 1. Terminate entire process trees by PID (bounded: `taskkill /F /T` can
    // stall on an uninterruptible process — pre-flight must never hang on it)
    for pid in &unique_pids {
        crate::crash_handler::run_bounded("taskkill", &["/F", "/T", "/PID", &pid.to_string()], 8000);
    }

    // 2. Terminate matching executable images
    for exe in &unique_exes {
        crate::crash_handler::run_bounded("taskkill", &["/F", "/T", "/IM", exe], 8000);
        eprintln!("[PRE-FLIGHT] Auto-terminated process image: {}", exe);
    }

    // 3. Win32 TerminateProcess fallback for any stubborn process handles
    for pid in &unique_pids {
        unsafe {
            if let Ok(hproc) = OpenProcess(PROCESS_TERMINATE, false, *pid) {
                let _ = TerminateProcess(hproc, 1);
                let _ = CloseHandle(hproc);
            }
        }
    }
}

/// Runs the complete strict pre-flight scan & clean cycle:
/// 1. Suppresses Bluetooth service automatically.
/// 2. Performs automated application termination passes (closing all in our hand).
/// 3. Rescans the system.
/// 4. If any applications persist, prompts the user with the exact list.
/// 5. Loops and rescans until ZERO running applications remain. NEVER moves forward while apps are alive.
pub fn enforce_clean_environment(is_production: bool) -> bool {
    let own_pid = unsafe { GetCurrentProcessId() };

    eprintln!("[PRE-FLIGHT] Starting Pre-Launch Environment Enforcement (Production Mode: {})...", is_production);

    // Step 1: Suppress Bluetooth hardware & service immediately (bounded —
    // `net stop` must never stall the pre-flight gate)
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

    // Step 3: Strict Verification Loop  Prompts candidate and loops until ZERO applications remain
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
            eprintln!("[PRE-FLIGHT] Candidate clicked Cancel. Aborting pre-flight safely. Restoring Bluetooth service.");
            crate::crash_handler::run_bounded("net", &["start", "bthserv"], 5000);
            return false;
        }

        // When user clicks OK, try one more auto-kill attempt on whatever is left, then sleep and loop to rescan
        terminate_detected_applications(&still_running);
        std::thread::sleep(Duration::from_millis(600));

        // The loop returns to the top and calls `scan_running_applications` again.
        // If still_running is still not empty, it WILL prompt again!
        // It will NEVER advance until `still_running.is_empty()` is true!
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
        })
        .collect();
    terminate_detected_applications(&apps);
}
