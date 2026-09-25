//! CITADEL Client Module: Failsafe Crash Recovery & Panic Handler
//!
//! Ensures that if the client process encounters an unhandled panic, OS termination
//! signal, or unexpected crash:
//! 1. Registry escape locks (DisableTaskMgr, NoWinKeys, etc.) are erased/restored.
//! 2. Display plane is switched back to the default desktop.
//! 3. Windows Explorer shell (explorer.exe) is restarted.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::process::Command;

use windows::core::PCWSTR;
use windows::Win32::Foundation::BOOL;
use windows::Win32::System::Console::SetConsoleCtrlHandler;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_WRITE,
};
use windows::Win32::System::StationsAndDesktops::{
    OpenDesktopW, SetThreadDesktop, SwitchDesktop, DESKTOP_CONTROL_FLAGS,
};

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

pub fn emergency_restore_system() {
    eprintln!("[CITADEL EMERGENCY] Initiating failsafe system restoration...");

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

    // 3. Restart explorer.exe
    let _ = Command::new("explorer.exe").spawn();

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
