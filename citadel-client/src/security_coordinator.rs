//! CITADEL Client Module: High-Assurance Security Coordinator
//!
//! Orchestrates the multi-layered kiosk lockdown architecture:
//! 1. Failsafe crash and panic handlers
//! 2. Registry policy hardening (DisableTaskMgr, NoWinKeys, etc.)
//! 3. Kernel WFP zero-internet network isolation
//! 4. Explorer shell termination & watchdog
//! 5. Secure Win32 desktop creation & display switch
//! 6. System keyboard hook with 5-second health watchdog
//! 7. Active anti-cheat sensors (M2 loopback, M4 capture-exclusion, M5 injection)
//! 8. Clipboard flusher and background process watchdog

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
use crate::hotkey_lock::{install_hotkey_lock_with_desktop, HotkeyLockHandle};
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
    // Fields are dropped in declaration order:
    // 1. Hotkey handle unhooks and stops health watchdog
    _hotkey_handle: HotkeyLockHandle,
    // 2. Secure desktop switches back to Default and closes HDESK
    _secure_desktop: SecureDesktop,
    // 3. Explorer lock stops watchdog and relaunches explorer.exe
    _explorer_lock: ExplorerLock,
    // 4. Auxiliary shell and input guards drop
    _taskbar_lock: TaskbarLock,
    _touchpad_lock: TouchpadLock,
    _foreground_lock: ForegroundLock,
    _clipboard_guard: ClipboardGuard,
    _process_watchdog: ProcessWatchdog,
    // 5. WFP engine removes kernel firewall rules
    _wfp_engine: WfpEngine,
    // 6. Registry lock restores Task Manager, WinKeys, Lock, etc.
    _registry_lock: RegistryLock,

    stop_signal: Arc<AtomicBool>,
    sensor_thread: Option<JoinHandle<()>>,
    violations: Arc<Mutex<Vec<String>>>,
    server_ip: Ipv4Addr,
    server_port: u16,
}

impl ClientLockdownGuard {
    /// Initializes full-system kiosk lockdown.
    /// Requires Administrator privilege — fails if elevation is not present.
    pub fn new(server_ip: Ipv4Addr, server_port: u16) -> Result<Self, String> {
        if !is_elevated() {
            return Err("CITADEL Lockdown requires Administrator privileges to engage kernel network filtering and hardware lock.".to_string());
        }

        // 0. Install failsafe panic & console close crash recovery handlers
        install_crash_safety();

        let violations = Arc::new(Mutex::new(Vec::new()));
        let stop_signal = Arc::new(AtomicBool::new(false));

        // 1. FIRST: Registry Hardening (neutering Task Manager, Lock, Sign-Out before anything else)
        let registry_lock = RegistryLock::acquire()
            .map_err(|e| format!("Failed to apply registry security policies: {}", e))?;

        // 2. SECOND: Install WFP Zero-Internet Filter in Windows Kernel
        let mut wfp_engine = WfpEngine::open_dynamic()
            .map_err(|e| format!("Failed to open WFP engine: {:?}", e))?;

        wfp_engine.install_college_lan_policy(server_ip, server_port)
            .map_err(|e| format!("Failed to apply WFP zero-internet firewall rule: {:?}", e))?;

        eprintln!(
            "[CITADEL CLIENT] HARDWARE NETWORK LOCK ACTIVE: All public internet dropped. Permitted server: {}:{}",
            server_ip, server_port
        );

        // 3. THIRD: Kill Explorer shell & start explorer watchdog
        let explorer_lock = ExplorerLock::acquire();

        // 4. FOURTH: Create and switch to isolated Win32 Secure Desktop
        let secure_desktop = SecureDesktop::create_and_switch()
            .map_err(|e| format!("Failed to create isolated Secure Desktop: {}", e))?;

        // 5. FIFTH: Install Keyboard Hook with Health Watchdog on the Secure Desktop
        let hotkey_handle = install_hotkey_lock_with_desktop(Some(secure_desktop.handle()))
            .map_err(|e| format!("Failed to install hotkey suppression hook: {}", e))?;

        // 6. Auxiliary input & shell guards
        let taskbar_lock = TaskbarLock::acquire();
        let touchpad_lock = TouchpadLock::acquire();
        let foreground_lock = ForegroundLock::start();
        let clipboard_guard = ClipboardGuard::start();
        let process_watchdog = ProcessWatchdog::start(violations.clone());

        // 7. Background anti-cheat sensor thread (M2 loopback, M4 capture-exclusion, M5 injection)
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
            _hotkey_handle: hotkey_handle,
            _secure_desktop: secure_desktop,
            _explorer_lock: explorer_lock,
            _taskbar_lock: taskbar_lock,
            _touchpad_lock: touchpad_lock,
            _foreground_lock: foreground_lock,
            _clipboard_guard: clipboard_guard,
            _process_watchdog: process_watchdog,
            _wfp_engine: wfp_engine,
            _registry_lock: registry_lock,
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

    pub fn secure_desktop_name(&self) -> &str {
        self._secure_desktop.name()
    }

    /// Spawns the locked kiosk browser directly on the isolated secure desktop plane.
    pub fn launch_browser(&self) -> Result<KioskProcess, String> {
        let endpoint = self.server_endpoint();
        launch_kiosk_on_desktop(&endpoint, Some(self.secure_desktop_name()))
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
