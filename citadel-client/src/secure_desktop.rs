//! CITADEL Client Module: Windows Secure Desktop Isolation
//!
//! Provides OS-level visual and execution isolation equivalent to Safe Exam Browser (SEB).
//!
//! By creating and switching to a dedicated Win32 Desktop object:
//! 1. Explorer shell, Taskbar, Start menu, and notification popups DO NOT EXIST on this desktop.
//! 2. Alt-Tab, Win-Tab, and Task View cannot see or switch to applications on the default desktop.
//! 3. Touchpad gestures cannot switch to virtual desktops because this desktop is isolated.
//! 4. Screen capture tools on other desktops cannot view the exam surface.
//!
//! On exit (or crash), the client safely switches back to the original default desktop
//! and closes the secure desktop handle.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::GENERIC_ALL;
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, CreateDesktopW, GetThreadDesktop, SetThreadDesktop, SwitchDesktop,
    DESKTOP_CONTROL_FLAGS, HDESK,
};
use windows::Win32::System::Threading::{
    CreateProcessW, GetCurrentProcessId, GetCurrentThreadId, CREATE_NEW_PROCESS_GROUP,
    PROCESS_INFORMATION, STARTF_USESHOWWINDOW, STARTUPINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_MAXIMIZE;

pub struct SecureDesktop {
    original_desktop: HDESK,
    secure_desktop: HDESK,
    desktop_name: String,
}

fn to_wide_null(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

impl SecureDesktop {
    /// Creates a new isolated desktop and switches display output to it.
    pub fn create_and_switch() -> Result<Self, String> {
        unsafe {
            // 1. Capture current desktop handle so we can cleanly return on exit
            let thread_id = GetCurrentThreadId();
            let original_desktop = GetThreadDesktop(thread_id)
                .map_err(|e| format!("Failed to get current thread desktop: {:?}", e))?;

            // 2. Generate a unique secure desktop name
            let pid = GetCurrentProcessId();
            let desktop_name = format!("CitadelSecureDesktop_{}", pid);
            let wide_name = to_wide_null(&desktop_name);

            // 3. Create dedicated desktop with full access mask
            let secure_desktop = CreateDesktopW(
                PCWSTR(wide_name.as_ptr()),
                PCWSTR::null(),
                None,
                DESKTOP_CONTROL_FLAGS(0),
                GENERIC_ALL.0,
                None,
            ).map_err(|e| format!("CreateDesktopW failed: {:?}", e))?;

            // 4. Associate the calling thread with the secure desktop so any window it creates appears there
            SetThreadDesktop(secure_desktop)
                .map_err(|e| format!("SetThreadDesktop on secure desktop failed: {:?}", e))?;

            // 5. Switch physical screen output to the secure desktop
            SwitchDesktop(secure_desktop)
                .map_err(|e| format!("SwitchDesktop failed: {:?}", e))?;

            eprintln!(
                "[CITADEL CLIENT] SECURE DESKTOP ACTIVE: Isolated display plane '{}' created and switched.",
                desktop_name
            );

            Ok(SecureDesktop {
                original_desktop,
                secure_desktop,
                desktop_name,
            })
        }
    }

    /// Desktop name identifier for process startup info.
    pub fn name(&self) -> &str {
        &self.desktop_name
    }

    /// Returns the HDESK handle of the secure desktop.
    pub fn handle(&self) -> HDESK {
        self.secure_desktop
    }

    /// Launches an isolated application directly onto this secure desktop.
    pub fn launch_on_desktop(&self, exe_path: &Path, args: &[&str]) -> Result<PROCESS_INFORMATION, String> {
        unsafe {
            let mut wide_desktop = to_wide_null(&self.desktop_name);
            let mut si = STARTUPINFOW::default();
            si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
            si.lpDesktop = PWSTR(wide_desktop.as_mut_ptr());
            si.dwFlags = STARTF_USESHOWWINDOW;
            si.wShowWindow = SW_MAXIMIZE.0 as u16;

            let exe_str = exe_path.to_string_lossy();
            let full_command = format!("\"{}\" {}", exe_str, args.join(" "));
            let mut wide_cmd = to_wide_null(&full_command);

            let mut pi = PROCESS_INFORMATION::default();

            CreateProcessW(
                PCWSTR::null(),
                PWSTR(wide_cmd.as_mut_ptr()),
                None,
                None,
                false,
                CREATE_NEW_PROCESS_GROUP,
                None,
                PCWSTR::null(),
                &si,
                &mut pi,
            ).map_err(|e| format!("CreateProcessW on secure desktop failed: {:?}", e))?;

            Ok(pi)
        }
    }

    /// Restores the default user desktop and closes the secure desktop.
    pub fn restore(&mut self) {
        unsafe {
            if !self.original_desktop.is_invalid() {
                let _ = SwitchDesktop(self.original_desktop);
                let _ = SetThreadDesktop(self.original_desktop);
            }
            if !self.secure_desktop.is_invalid() {
                let _ = CloseDesktop(self.secure_desktop);
                self.secure_desktop = HDESK::default();
            }
        }
        eprintln!("[CITADEL CLIENT] SECURE DESKTOP RELEASED: Returned to original Windows desktop.");
    }
}

impl Drop for SecureDesktop {
    fn drop(&mut self) {
        self.restore();
    }
}
