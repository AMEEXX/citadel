//! CITADEL Pre-Launch Environment Scanner & Enforcement
//!
//! Scans running workstation processes, forcefully terminates prohibited applications
//! (browsers, chat, screen recorders, cheat tools), and strictly verifies the environment
//! is 100% clean before the exam kiosk opens.
//!
//! Zero tolerance: The candidate CANNOT bypass or cancel this check. The verification
//! loop repeats indefinitely until ALL prohibited processes are eliminated.

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
    MessageBoxW, MESSAGEBOX_STYLE, MB_ICONWARNING, MB_OK,
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
    "msedge.exe",
    "edge.exe",
    "iexplore.exe",

    // Communication & Collaboration
    "discord.exe",
    "slack.exe",
    "telegram.exe",
    "whatsapp.exe",
    "teams.exe",
    "skype.exe",
    "signal.exe",
    "zoom.exe",

    // Remote Desktop & Screen Sharing
    "teamviewer.exe",
    "anydesk.exe",
    "rustdesk.exe",
    "vncviewer.exe",
    "ultraviewer.exe",
    "parsec.exe",
    "mstsc.exe",

    // Cheats, AI, & Screen Capture
    "obs64.exe",
    "obs32.exe",
    "cheatengine.exe",
    "cheatengine-x86_64.exe",
    "snippingtool.exe",
    "screenclippinghost.exe",
    "ollama.exe",
    "lmstudio.exe",
    "chatgpt.exe",
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

                // Never terminate own process, recovery tools, citadel server, or webview2 runtime
                let is_citadel_internal = pid == own_pid
                    || exe_name.contains("citadel")
                    || exe_name.contains("recovery")
                    || exe_name.contains("msedgewebview2");

                if !is_citadel_internal {
                    // In testing mode, preserve developer environment (antigravity, rustc, cargo, vscode)
                    let is_dev_tool = exe_name.contains("antigravity")
                        || exe_name.contains("cargo")
                        || exe_name.contains("rustc")
                        || exe_name.contains("powershell")
                        || exe_name.contains("cmd.exe");

                    if !is_production && is_dev_tool {
                        // Skip dev tool in non-production testing
                    } else {
                        for &prohibited in PROHIBITED_PROCESSES {
                            if exe_name == prohibited || (exe_name.contains(prohibited) && !exe_name.contains("msedgewebview2")) {
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
                eprintln!("[PRE-FLIGHT] Auto-terminated prohibited process: {} (PID: {})", name, pid);
            }
        }
    }
}

/// Runs the complete strict pre-flight scan & clean cycle.
/// Zero tolerance: Automatically terminates detected apps first. If any survive, prompts candidate
/// to close them manually and will NEVER proceed until 100% of prohibited processes are closed.
pub fn enforce_clean_environment(is_production: bool) -> bool {
    let own_pid = unsafe { GetCurrentProcessId() };

    // Step 1: Initial auto-kill scan
    let detected = scan_prohibited_processes(own_pid, is_production);
    if !detected.is_empty() {
        eprintln!("[PRE-FLIGHT] Detected {} prohibited application(s). Initiating auto-termination...", detected.len());
        terminate_prohibited_processes(&detected);
        std::thread::sleep(Duration::from_millis(800));
    }

    // Step 2: Strict, infinite verification loop — NO ESCAPE until workstation is clean
    loop {
        let remaining = scan_prohibited_processes(own_pid, is_production);
        if remaining.is_empty() {
            eprintln!("[PRE-FLIGHT] Workstation verified 100% clean. Ready for exam launch.");
            break;
        }

        // Secondary automatic kill attempt
        terminate_prohibited_processes(&remaining);
        std::thread::sleep(Duration::from_millis(600));

        let still_running = scan_prohibited_processes(own_pid, is_production);
        if still_running.is_empty() {
            eprintln!("[PRE-FLIGHT] Workstation clean after secondary kill pass.");
            break;
        }

        // Still running: Candidate MUST close them manually. Dialog has NO Cancel button.
        let mut remaining_names: Vec<String> = still_running
            .iter()
            .map(|(name, pid)| format!("  • {} (PID: {})", name, pid))
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        remaining_names.sort();

        let list_str = remaining_names.join("\n");
        let warn_msg = format!(
            "CITADEL Security Boundary: Prohibited Applications Running\n\n            The following applications are running and could not be terminated automatically:\n\n            {}\n\n            CITADEL policy requires that ALL external applications, browsers, communication tools,\n            and screen sharing software MUST be closed before the assessment environment can open.\n\n            Please manually close these applications from your taskbar or Task Manager.\n\n            Click 'OK' after closing them to re-scan and verify.",
            list_str
        );

        // MB_OK with MB_ICONWARNING: Candidate cannot click Cancel. They must click OK to re-scan.
        let _ = show_dialog("CITADEL Security Verification Required", &warn_msg, MB_OK | MB_ICONWARNING);

        // Immediately try auto-terminating again after dialog dismiss
        terminate_prohibited_processes(&still_running);
        std::thread::sleep(Duration::from_millis(500));
    }

    true
}
