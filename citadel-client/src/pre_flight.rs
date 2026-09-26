//! CITADEL Pre-Launch Environment Scanner & Enforcement
//!
//! Scans running workstation processes, notifies candidate, forcefully terminates
//! prohibited applications (browsers, chat, screen recorders, cheat tools),
//! and verifies the environment is 100% clean before the exam kiosk opens.

use std::collections::HashSet;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::time::Duration;

use windows::core::PCWSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    GetCurrentProcessId, OpenProcess, TerminateProcess, PROCESS_TERMINATE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, MESSAGEBOX_STYLE, IDOK, MB_ICONINFORMATION, MB_ICONWARNING, MB_OKCANCEL,
};

pub const PROHIBITED_PROCESSES: &[&str] = &[
    // Web Browsers
    "chrome.exe",
    "firefox.exe",
    "brave.exe",
    "opera.exe",
    "opera_gx.exe",
    "vivaldi.exe",
    "tor.exe",
    // Communication & Collaboration
    "discord.exe",
    "slack.exe",
    "telegram.exe",
    "whatsapp.exe",
    "teams.exe",
    "skype.exe",
    "signal.exe",
    // Remote Desktop & Screen Sharing
    "teamviewer.exe",
    "anydesk.exe",
    "rustdesk.exe",
    "vncviewer.exe",
    "ultraviewer.exe",
    "parsec.exe",
    "zoom.exe",
    // Cheats, AI, & Screen Capture
    "obs64.exe",
    "obs32.exe",
    "cheatengine.exe",
    "cheatengine-x86_64.exe",
    "snippingtool.exe",
    "screenclippinghost.exe",
    "ollama.exe",
    "lmstudio.exe",
];

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

pub fn scan_prohibited_processes(own_pid: u32, is_production: bool) -> Vec<(String, u32)> {
    let mut detected = Vec::new();

    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return detected,
        };

        let mut entry = PROCESSENTRY32W::default();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let null_pos = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                let exe_name = OsString::from_wide(&entry.szExeFile[..null_pos])
                    .to_string_lossy()
                    .to_lowercase();

                let pid = entry.th32ProcessID;

                // Never flag or terminate own process, recovery tools, or citadel server
                if pid != own_pid && !exe_name.contains("citadel") && !exe_name.contains("recovery") {
                    // In testing mode, preserve developer environment (antigravity, rustc, cargo)
                    let is_dev_tool = exe_name.contains("antigravity") || exe_name.contains("cargo") || exe_name.contains("rustc");
                    if !is_production && is_dev_tool {
                        // Skip dev tool in non-production testing
                    } else {
                        for &prohibited in PROHIBITED_PROCESSES {
                            if exe_name == prohibited || (exe_name.contains(prohibited) && !exe_name.contains("citadel")) {
                                detected.push((exe_name.clone(), pid));
                                break;
                            }
                        }
                    }
                }

                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
    }

    detected
}

pub fn terminate_prohibited_processes(processes: &[(String, u32)]) {
    for (name, pid) in processes {
        unsafe {
            if let Ok(hproc) = OpenProcess(PROCESS_TERMINATE, false, *pid) {
                let _ = TerminateProcess(hproc, 1);
                let _ = CloseHandle(hproc);
                eprintln!("[PRE-FLIGHT] Terminated prohibited process: {} (PID: {})", name, pid);
            }
        }
    }
}

/// Runs the complete pre-flight scan & clean cycle.
/// Returns true if the environment is clean and ready for exam launch, false if candidate canceled.
pub fn enforce_clean_environment(is_production: bool) -> bool {
    let own_pid = unsafe { GetCurrentProcessId() };

    // Step 1: Initial scan
    let detected = scan_prohibited_processes(own_pid, is_production);

    if !detected.is_empty() {
        // Collect unique process names for clean presentation
        let mut unique_names: Vec<String> = detected
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        unique_names.sort();

        let list_str = unique_names
            .iter()
            .map(|name| format!("  • {}", name))
            .collect::<Vec<_>>()
            .join("\n");

        let msg = format!(
            "CITADEL Secure Assessment Environment\n\n\
            The following external applications are currently open on your workstation:\n\n\
            {}\n\n\
            To maintain exam integrity, all external applications must be closed before the exam environment can open.\n\n\
            Click 'OK' to automatically close these applications and proceed.\n\
            Click 'Cancel' to abort.",
            list_str
        );

        let choice = show_dialog("CITADEL Pre-Exam Environment Scan", &msg, MB_OKCANCEL | MB_ICONINFORMATION);
        if choice != IDOK {
            eprintln!("[PRE-FLIGHT] Candidate canceled environment cleanup. Aborting launch.");
            return false;
        }

        // Forcefully terminate detected apps
        terminate_prohibited_processes(&detected);
        std::thread::sleep(Duration::from_millis(800));
    }

    // Step 2: Verification Loop
    loop {
        let remaining = scan_prohibited_processes(own_pid, is_production);
        if remaining.is_empty() {
            eprintln!("[PRE-FLIGHT] Workstation verified clean. Ready for exam launch.");
            break;
        }

        // Attempt second cleanup pass
        terminate_prohibited_processes(&remaining);
        std::thread::sleep(Duration::from_millis(500));

        let still_running = scan_prohibited_processes(own_pid, is_production);
        if still_running.is_empty() {
            eprintln!("[PRE-FLIGHT] Workstation verified clean after secondary pass.");
            break;
        }

        let mut remaining_names: Vec<String> = still_running
            .iter()
            .map(|(name, pid)| format!("{} (PID: {})", name, pid))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        remaining_names.sort();

        let list_str = remaining_names
            .iter()
            .map(|s| format!("  • {}", s))
            .collect::<Vec<_>>()
            .join("\n");

        let warn_msg = format!(
            "Prohibited Applications Still Running\n\n\
            The following applications could not be closed automatically (they may be protected by Windows or running under another user):\n\n\
            {}\n\n\
            Please manually close these applications from your taskbar or Task Manager.\n\n\
            Click 'OK' to re-scan and verify.\n\
            Click 'Cancel' to abort exam launch.",
            list_str
        );

        let choice = show_dialog("CITADEL Application Verification", &warn_msg, MB_OKCANCEL | MB_ICONWARNING);
        if choice != IDOK {
            eprintln!("[PRE-FLIGHT] Candidate canceled during manual close verification. Aborting launch.");
            return false;
        }
    }

    true
}
