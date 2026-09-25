//! CITADEL Client Module: Explorer Shell Lockdown
//!
//! Kills and suppresses the Windows Shell (explorer.exe) process during the exam session.
//!
//! Even though the Secure Desktop runs in an isolated desktop object, disabling explorer.exe
//! eliminates:
//! 1. The default desktop's Taskbar and Start Menu.
//! 2. Shell hotkey handling (Win+E, Win+R, Win+X) at the OS shell level.
//! 3. Background notification flyouts and system tray popups.
//! 4. Any attempt by Task Manager or secondary utilities to respawn the desktop shell.
//!
//! On session exit or handled panic, the watchdog terminates and cleanly relaunches
//! explorer.exe to restore the student's normal desktop experience.

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

pub struct ExplorerLock {
    stop_signal: Arc<AtomicBool>,
    watchdog_thread: Option<JoinHandle<()>>,
}

impl ExplorerLock {
    /// Terminates existing explorer.exe processes and spawns a watchdog to kill any respawns.
    pub fn acquire() -> Self {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_signal.clone();

        // Initial termination
        Self::terminate_explorer_processes();

        let watchdog_thread = thread::spawn(move || {
            while !stop_clone.load(Ordering::Relaxed) {
                Self::terminate_explorer_processes();
                thread::sleep(Duration::from_millis(500));
            }
        });

        eprintln!("[CITADEL CLIENT] EXPLORER SHELL LOCK ACTIVE: explorer.exe suppressed with watchdog.");

        ExplorerLock {
            stop_signal,
            watchdog_thread: Some(watchdog_thread),
        }
    }

    /// Scans for and terminates any running explorer.exe processes.
    pub fn terminate_explorer_processes() {
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

                    if exe_name == "explorer.exe" {
                        let pid = entry.th32ProcessID;
                        if let Ok(hproc) = OpenProcess(PROCESS_TERMINATE, false, pid) {
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

    /// Restores the Windows Explorer shell by relaunching explorer.exe.
    pub fn restore(&mut self) {
        self.stop_signal.store(true, Ordering::Relaxed);
        if let Some(h) = self.watchdog_thread.take() {
            let _ = h.join();
        }

        // Relaunch Windows Explorer shell
        let _ = Command::new("explorer.exe").spawn();
        eprintln!("[CITADEL CLIENT] EXPLORER SHELL RESTORED: explorer.exe restarted.");
    }
}

impl Drop for ExplorerLock {
    fn drop(&mut self) {
        self.restore();
    }
}
