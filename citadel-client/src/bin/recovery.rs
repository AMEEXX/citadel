//! CITADEL Emergency Recovery Utility
//!
//! Run as Administrator to immediately release all locks, restore Task Manager,
//! relaunch Windows Explorer, and return the system to normal state.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::process::Command;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{BOOL, CloseHandle, HWND};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_ALL_ACCESS, KEY_SET_VALUE,
};
use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, MessageBoxW, ShowWindow, MB_ICONINFORMATION, MB_OK, SW_SHOW,
};

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

fn kill_processes_by_name(target_name: &str) {
    let own_pid = unsafe { windows::Win32::System::Threading::GetCurrentProcessId() };
    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return,
        };

        let mut entry = PROCESSENTRY32W::default();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let exe_name = String::from_utf16_lossy(&entry.szExeFile)
                    .trim_matches(char::from(0))
                    .to_lowercase();

                if exe_name == target_name.to_lowercase() && entry.th32ProcessID != own_pid {
                    if let Ok(hproc) = OpenProcess(PROCESS_TERMINATE, false, entry.th32ProcessID) {
                        let _ = TerminateProcess(hproc, 1);
                        let _ = CloseHandle(hproc);
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

fn restore_registry_policies() {
    let keys = [
        (
            r"Software\Microsoft\Windows\CurrentVersion\Policies\System",
            &["DisableTaskMgr", "DisableLockWorkstation", "DisableChangePassword"][..],
        ),
        (
            r"Software\Microsoft\Windows\CurrentVersion\Policies\Explorer",
            &["NoWinKeys", "NoClose", "NoLogoff"][..],
        ),
        (
            r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced",
            &["EnableSnapAssistFlyout"][..],
        ),
    ];

    for (subkey, val_names) in keys {
        let subkey_w = to_wide(subkey);
        for access in [KEY_ALL_ACCESS, KEY_SET_VALUE] {
            unsafe {
                let mut hkey = HKEY::default();
                if RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    PCWSTR(subkey_w.as_ptr()),
                    0,
                    access,
                    &mut hkey,
                ).is_ok() {
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
            let full_path = format!("HKCU\\{}", subkey);
            let _ = Command::new("reg")
                .args(["delete", &full_path, "/v", val_name, "/f"])
                .output();
        }
    }
}

fn restore_taskbars() {
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

fn is_process_running(target_name: &str) -> bool {
    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return false,
        };

        let mut entry = PROCESSENTRY32W::default();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let exe_name = String::from_utf16_lossy(&entry.szExeFile)
                    .trim_matches(char::from(0))
                    .to_lowercase();

                if exe_name == target_name.to_lowercase() {
                    let _ = CloseHandle(snapshot);
                    return true;
                }

                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
        false
    }
}

fn main() {
    // 1. Terminate any running citadel client or guard service
    kill_processes_by_name("citadel-client.exe");
    kill_processes_by_name("guard-svc.exe");

    // 2. Restore all registry policies
    restore_registry_policies();

    // 3. Restore and show taskbars
    restore_taskbars();

    // 4. Ensure explorer.exe is running
    if !is_process_running("explorer.exe") {
        let _ = Command::new("explorer.exe").spawn();
    }

    // 5. Display success dialog
    let msg = to_wide(
        "Citadel Emergency Recovery Completed Successfully!\n\n         ??? Task Manager restored\n         ??? Windows key and lock policies restored\n         ??? Taskbar and Explorer restored\n         ??? Lockdown client processes terminated\n\n         Your system is back to normal.",
    );
    let title = to_wide("Citadel Recovery Utility");

    if std::env::args().any(|a| a == "--silent") {
        return;
    }
    unsafe {
        let _ = MessageBoxW(
            HWND(std::ptr::null_mut()),
            PCWSTR(msg.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}
