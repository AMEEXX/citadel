//! CITADEL Client Security Coordinator
//!
//! Orchestrates the complete client-side kiosk lockdown lifecycle:
//! 1. UAC Administrator Elevation enforcement
//! 2. WFP dynamic kernel network isolation (zero internet, college server only)
//! 3. Low-level hotkey suppression (blocks Alt-Tab, Win Key, Ctrl-Esc, Alt-F4, etc.)
//! 4. Taskbar and Start button lock (continuous hiding and event disabling)
//! 5. Touchpad gesture suppression (3-finger and 4-finger swipes disabled via registry)
//! 6. Foreground window dominance (pins kiosk as HWND_TOPMOST)
//! 7. System clipboard isolation guard (periodic wiping of clipboard)
//! 8. Process watchdog (terminates blacklisted cheat tools)
//! 9. Background anti-cheat sensors (M2 loopback, M4 capture-exclusion, M5 injection)
//! 10. Safe RAII cleanup upon exit

use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use guard_net::WfpEngine;
use guard_svc::llm_detect;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::hotkey_lock::{install_hotkey_lock, HotkeyLockHandle};
use crate::kiosk_window::{
    ClipboardGuard, ForegroundLock, ProcessWatchdog, TaskbarLock, TouchpadLock,
};

/// Checks if the current process is running with elevated Administrator privileges.
pub fn is_elevated() -> bool {
    let mut handle = Default::default();
    unsafe {
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle).is_ok() {
            let mut elevation = TOKEN_ELEVATION::default();
            let mut size = 0;
            if GetTokenInformation(
                handle,
                TokenElevation,
                Some(&mut elevation as *mut _ as *mut _),
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut size,
            ).is_ok() {
                return elevation.TokenIsElevated != 0;
            }
        }
    }
    false
}

/// Triggers a Windows UAC prompt to relaunch this application as Administrator.
pub fn elevate_self(args: &[String]) -> Result<(), String> {
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_path_str = current_exe.to_string_lossy();
    let wide_exe: Vec<u16> = exe_path_str.encode_utf16().chain(std::iter::once(0)).collect();

    // Reconstruct arguments to pass to the elevated instance
    let args_str = args.join(" ");
    let wide_args: Vec<u16> = args_str.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let result = ShellExecuteW(
            HWND(std::ptr::null_mut()),
            w!("runas"),
            PCWSTR(wide_exe.as_ptr()),
            PCWSTR(wide_args.as_ptr()),
            PCWSTR(std::ptr::null()),
            SW_SHOWNORMAL,
        );

        if result.0 as usize > 32 {
            Ok(())
        } else {
            Err(format!("UAC elevation request declined by user (code {})", result.0 as usize))
        }
    }
}

pub struct ClientLockdownGuard {
    _wfp_engine: WfpEngine,
    _hotkey_handle: HotkeyLockHandle,
    _taskbar_lock: TaskbarLock,
    _touchpad_lock: TouchpadLock,
    _foreground_lock: ForegroundLock,
    _clipboard_guard: ClipboardGuard,
    _process_watchdog: ProcessWatchdog,
    stop_signal: Arc<AtomicBool>,
    sensor_thread: Option<JoinHandle<()>>,
    violations: Arc<Mutex<Vec<String>>>,
    server_ip: Ipv4Addr,
    server_port: u16,
}

impl ClientLockdownGuard {
    /// Initializes client-side lockdown for the target exam server.
    /// Strictly requires Administrator privilege — fails if elevation is not present.
    pub fn new(server_ip: Ipv4Addr, server_port: u16) -> Result<Self, String> {
        if !is_elevated() {
            return Err("CITADEL Lockdown requires Administrator privileges to engage kernel network filtering and hardware lock.".to_string());
        }

        let violations = Arc::new(Mutex::new(Vec::new()));
        let stop_signal = Arc::new(AtomicBool::new(false));

        // 1. Install WFP Zero-Internet Filter in Windows Kernel
        let mut wfp_engine = WfpEngine::open_dynamic()
            .map_err(|e| format!("Failed to open WFP engine: {:?}", e))?;

        wfp_engine.install_college_lan_policy(server_ip, server_port)
            .map_err(|e| format!("Failed to apply WFP zero-internet firewall rule: {:?}", e))?;

        eprintln!(
            "[CITADEL CLIENT] HARDWARE NETWORK LOCK ACTIVE: All public internet dropped. Permitted server: {}:{}",
            server_ip, server_port
        );

        // 2. Install Hotkey Suppression Hook (Win key, Alt-Tab, Ctrl-Esc, PrtSc, Alt-F4, etc.)
        let hotkey_handle = install_hotkey_lock()
            .map_err(|e| format!("Failed to install hotkey suppression hook: {}", e))?;

        eprintln!("[CITADEL CLIENT] SYSTEM KEYBOARD HOOK ACTIVE: Alt-Tab, Win Key, Ctrl-Esc, PrtSc intercepted.");

        // 3. Lock Taskbar and Start Menu (hide & disable click events)
        let taskbar_lock = TaskbarLock::acquire();
        eprintln!("[CITADEL CLIENT] TASKBAR LOCK ACTIVE: Shell taskbar and start menu suppressed.");

        // 4. Suppress Precision Touchpad 3-finger and 4-finger gestures
        let touchpad_lock = TouchpadLock::acquire();
        eprintln!("[CITADEL CLIENT] TOUCHPAD LOCK ACTIVE: Multi-finger gestures suppressed.");

        // 5. Enforce Foreground Window Dominance
        let foreground_lock = ForegroundLock::start();
        eprintln!("[CITADEL CLIENT] FOREGROUND LOCK ACTIVE: Kiosk pinned to HWND_TOPMOST.");

        // 6. Enforce Clipboard Isolation
        let clipboard_guard = ClipboardGuard::start();
        eprintln!("[CITADEL CLIENT] CLIPBOARD GUARD ACTIVE: System clipboard flusher running.");

        // 7. Start Process Watchdog (killing blacklisted cheat processes)
        let process_watchdog = ProcessWatchdog::start(violations.clone());
        eprintln!("[CITADEL CLIENT] PROCESS WATCHDOG ACTIVE: Blacklisted process killer running.");

        // 8. Start background anti-cheat sensor thread (M2 loopback, M4 capture-exclusion, M5 injection)
        let stop_clone = stop_signal.clone();
        let viol_clone = violations.clone();
        let sensor_thread = thread::spawn(move || {
            let allowed_ports = [server_port];
            while !stop_clone.load(Ordering::Relaxed) {
                // M2: Loopback Listener scan
                if let Ok(listeners) = llm_detect::scan_loopback_listeners() {
                    let v_list = llm_detect::check_listener_violations(&listeners, &allowed_ports);
                    for v in v_list {
                        let line = v.to_log_line();
                        eprintln!("[SECURITY VIOLATION] {}", line);
                        if let Ok(mut lock) = viol_clone.lock() {
                            lock.push(line);
                        }
                    }
                }

                // M4: Capture-Exclusion Window scan
                let excluded_windows = llm_detect::scan_capture_exclusion_windows();
                for w in excluded_windows {
                    let line = w.to_log_line();
                    eprintln!("[SECURITY VIOLATION] {}", line);
                    if let Ok(mut lock) = viol_clone.lock() {
                        lock.push(line);
                    }
                }

                // M5: Injected Keystrokes
                let injected = llm_detect::take_injected_keystroke_violations();
                for k in injected {
                    let line = k.to_log_line();
                    eprintln!("[SECURITY VIOLATION] {}", line);
                    if let Ok(mut lock) = viol_clone.lock() {
                        lock.push(line);
                    }
                }

                thread::sleep(Duration::from_secs(2));
            }
        });

        Ok(ClientLockdownGuard {
            _wfp_engine: wfp_engine,
            _hotkey_handle: hotkey_handle,
            _taskbar_lock: taskbar_lock,
            _touchpad_lock: touchpad_lock,
            _foreground_lock: foreground_lock,
            _clipboard_guard: clipboard_guard,
            _process_watchdog: process_watchdog,
            stop_signal,
            sensor_thread: Some(sensor_thread),
            violations,
            server_ip,
            server_port,
        })
    }

    pub fn server_endpoint(&self) -> String {
        format!("http://{}:{}/?token=citadel-secured-session", self.server_ip, self.server_port)
    }

    pub fn get_violations(&self) -> Vec<String> {
        self.violations.lock().unwrap().clone()
    }
}

impl Drop for ClientLockdownGuard {
    fn drop(&mut self) {
        eprintln!("[CITADEL CLIENT] Releasing client lockdown and restoring normal desktop & network...");
        self.stop_signal.store(true, Ordering::Relaxed);
        if let Some(thread) = self.sensor_thread.take() {
            let _ = thread.join();
        }
    }
}
