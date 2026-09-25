//! CITADEL Client Module: High-Assurance Security Coordinator
//!
//! Orchestrates the multi-layered kiosk lockdown architecture:
//! 1. Failsafe crash and panic handlers
//! 2. Safe deferred desktop preparation (creates isolated desktop in background)
//! 3. Kiosk browser launch & startup health verification
//! 4. Full OS-level lockdown engagement ONLY after browser is verified alive:
//!    - Registry policy hardening (DisableTaskMgr, NoWinKeys, etc.)
//!    - Kernel WFP zero-internet network isolation
//!    - Explorer shell termination & watchdog
//!    - Physical display plane switch to Secure Desktop
//!    - System keyboard hook with health watchdog
//!    - Active anti-cheat sensors (M2 loopback, M4 capture-exclusion, M5 injection)
//!    - Clipboard flusher and background process watchdog

use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use guard_net::WfpEngine;
use guard_svc::llm_detect;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Security::{
    GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::crash_handler::install_crash_safety;
use crate::explorer_lock::ExplorerLock;
use crate::hotkey_lock::{install_hotkey_lock, install_hotkey_lock_with_desktop, HotkeyLockHandle};
use crate::kiosk_window::{
    launch_kiosk_on_desktop, ClipboardGuard, ForegroundLock, KioskProcess, ProcessWatchdog,
    TaskbarLock, TouchpadLock,
};
use crate::registry_lock::RegistryLock;
use crate::secure_desktop::SecureDesktop;

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
    // Fields are dropped in declaration order on exit/panic:
    // 1. Hotkey handle unhooks and stops health watchdog
    _hotkey_handle: Option<HotkeyLockHandle>,
    // 2. Secure desktop switches back to Default desktop and closes HDESK (if used)
    _secure_desktop: Option<SecureDesktop>,
    // 3. Explorer lock stops watchdog and relaunches explorer.exe
    _explorer_lock: Option<ExplorerLock>,
    // 4. Auxiliary shell and input guards drop
    _taskbar_lock: Option<TaskbarLock>,
    _touchpad_lock: Option<TouchpadLock>,
    _foreground_lock: Option<ForegroundLock>,
    _clipboard_guard: Option<ClipboardGuard>,
    _process_watchdog: Option<ProcessWatchdog>,
    // 5. WFP engine removes kernel firewall rules
    _wfp_engine: Option<WfpEngine>,
    // 6. Registry lock restores Task Manager, WinKeys, Lock, etc.
    _registry_lock: Option<RegistryLock>,

    stop_signal: Arc<AtomicBool>,
    sensor_thread: Option<JoinHandle<()>>,
    violations: Arc<Mutex<Vec<String>>>,
    server_ip: Ipv4Addr,
    server_port: u16,
}

impl ClientLockdownGuard {
    /// Prepares the client lockdown in the background without affecting the user's display.
    /// Requires Administrator privileges.
    pub fn new(server_ip: Ipv4Addr, server_port: u16) -> Result<Self, String> {
        Self::new_with_mode(server_ip, server_port, false)
    }

    pub fn new_with_mode(server_ip: Ipv4Addr, server_port: u16, use_isolated_desktop: bool) -> Result<Self, String> {
        install_crash_safety();

        let violations = Arc::new(Mutex::new(Vec::new()));
        let stop_signal = Arc::new(AtomicBool::new(false));

        let secure_desktop = if use_isolated_desktop {
            if is_elevated() {
                Some(SecureDesktop::create().map_err(|e| format!("Failed to create isolated Secure Desktop: {}", e))?)
            } else {
                None
            }
        } else {
            None
        };

        Ok(ClientLockdownGuard {
            _hotkey_handle: None,
            _secure_desktop: secure_desktop,
            _explorer_lock: None,
            _taskbar_lock: None,
            _touchpad_lock: None,
            _foreground_lock: None,
            _clipboard_guard: None,
            _process_watchdog: None,
            _wfp_engine: None,
            _registry_lock: None,
            stop_signal,
            sensor_thread: None,
            violations,
            server_ip,
            server_port,
        })
    }

    pub fn server_endpoint(&self) -> String {
        format!("http://{}:{}/?token=citadel-secured-session", self.server_ip, self.server_port)
    }

    pub fn secure_desktop_name(&self) -> Option<&str> {
        self._secure_desktop.as_ref().map(|sd| sd.name())
    }

    pub fn launch_browser(&mut self) -> Result<KioskProcess, String> {
        let endpoint = self.server_endpoint();

        eprintln!("[CITADEL CLIENT] Launching exam kiosk browser window...");
        let kiosk_child = launch_kiosk_on_desktop(&endpoint, self.secure_desktop_name())?;

        // 2. If elevated, engage registry policies & kernel WFP firewall
        if is_elevated() {
            if let Ok(registry_lock) = RegistryLock::acquire() {
                self._registry_lock = Some(registry_lock);
            }
            if let Ok(mut wfp_engine) = WfpEngine::open_dynamic() {
                if wfp_engine.install_college_lan_policy(self.server_ip, self.server_port).is_ok() {
                    eprintln!(
                        "[CITADEL CLIENT] HARDWARE NETWORK LOCK ACTIVE: All public internet dropped. Permitted server: {}:{}",
                        self.server_ip, self.server_port
                    );
                    self._wfp_engine = Some(wfp_engine);
                }
            }
        }

        // 3. Desktop and shell handling
        if let Some(ref mut sd) = self._secure_desktop {
            self._explorer_lock = Some(ExplorerLock::acquire());
            sd.switch_to_secure()
                .map_err(|e| format!("Failed to switch physical display to Secure Desktop: {}", e))?;
            let hotkey_handle = install_hotkey_lock_with_desktop(Some(sd.handle()))
                .map_err(|e| format!("Failed to install hotkey suppression hook: {}", e))?;
            self._hotkey_handle = Some(hotkey_handle);
        } else {
            // Interactive Kiosk Mode (Safe, instant display, no black screen)
            self._taskbar_lock = Some(TaskbarLock::acquire());
            if let Ok(hotkey_handle) = install_hotkey_lock() {
                self._hotkey_handle = Some(hotkey_handle);
            }
        }

        // 7. Auxiliary input & shell guards
        self._taskbar_lock = Some(TaskbarLock::acquire());
        self._touchpad_lock = Some(TouchpadLock::acquire());
        self._foreground_lock = Some(ForegroundLock::start());
        self._clipboard_guard = Some(ClipboardGuard::start());
        self._process_watchdog = Some(ProcessWatchdog::start(self.violations.clone()));

        // 8. Background anti-cheat sensor thread (M2 loopback, M4 capture-exclusion, M5 injection)
        let stop_clone = self.stop_signal.clone();
        let viol_clone = self.violations.clone();
        let server_port = self.server_port;
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
        self.sensor_thread = Some(sensor_thread);

        Ok(kiosk_child)
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
