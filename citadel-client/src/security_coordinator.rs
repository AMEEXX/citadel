//! CITADEL Client Module: High-Assurance Security Coordinator
//!
//! Orchestrates the multi-layered kiosk lockdown architecture:
//! 1. Failsafe crash and panic handlers
//! 2. Safe deferred desktop preparation (creates isolated desktop in background)
//! 3. Kiosk browser launch & startup health verification
//! 4. Full OS-level lockdown engagement:
//!    - Registry policy hardening (DisableTaskMgr, NoWinKeys, etc.)
//!    - Kernel WFP zero-internet network isolation
//!    - Explorer shell termination & watchdog
//!    - Physical display plane switch to Secure Desktop (if requested)
//!    - System keyboard hook with health watchdog
//!    - Active anti-cheat sensors (M2 loopback, M4 capture-exclusion, M5 injection)
//!    - Clipboard flusher and background process watchdog

use std::net::Ipv4Addr;
use std::os::windows::ffi::OsStrExt;
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

/// Attempts to relaunch the current executable with elevated Administrator privileges
/// via the Windows Shell UAC dialog (runas).
pub fn elevate_self(args: &[String]) -> Result<(), String> {
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let args_str = args.join(" ");

    let wide_exe: Vec<u16> = current_exe.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
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
    pub fn new(server_ip: Ipv4Addr, server_port: u16) -> Result<Self, String> {
        Self::new_with_mode(server_ip, server_port, false, false)
    }

    pub fn new_with_mode(server_ip: Ipv4Addr, server_port: u16, use_isolated_desktop: bool, is_production: bool) -> Result<Self, String> {
        install_crash_safety();

        let violations = Arc::new(Mutex::new(Vec::new()));
        let stop_signal = Arc::new(AtomicBool::new(false));

        // === 1. Host Desktop Protection: Host registry policies are strictly NEVER touched ===
        let registry_lock = None;
        eprintln!("[CITADEL CLIENT] Host desktop protection active: Primary desktop registry policies preserved.");

        // === 2. ALWAYS install system-wide low-level keyboard hook immediately ===
        let hotkey_handle = match install_hotkey_lock() {
            Ok(hk) => {
                eprintln!("[CITADEL CLIENT] KEYBOARD HOOK ACTIVE: Alt+Tab, Win keys, system combos blocked.");
                Some(hk)
            }
            Err(e) => {
                eprintln!("[CITADEL CLIENT] Warning: Keyboard hook failed: {}", e);
                None
            }
        };

        // === 3. ALWAYS hide and suppress taskbars immediately ===
        let taskbar_lock = Some(TaskbarLock::acquire());

        // === 4. ALWAYS wipe clipboard continuously ===
        let clipboard_guard = Some(ClipboardGuard::start());

        // === 5. ALWAYS suppress multi-finger touchpad gestures ===
        let touchpad_lock = Some(TouchpadLock::acquire());

        // === 6. If elevated, engage kernel WFP network firewall ===
        let is_local_test = server_ip.is_loopback();
        let enforce_network = is_production
            || std::env::var("CITADEL_ENFORCE_NETWORK").map(|v| v == "1").unwrap_or(false);

        let wfp_engine = if is_elevated() && enforce_network && !is_local_test {
            match WfpEngine::open_dynamic() {
                Ok(mut engine) => {
                    if engine.install_college_lan_policy(server_ip, server_port).is_ok() {
                        eprintln!(
                            "[CITADEL CLIENT] HARDWARE NETWORK LOCK ACTIVE: All public internet dropped. Permitted server: {}:{}",
                            server_ip, server_port
                        );
                        Some(engine)
                    } else {
                        eprintln!("[CITADEL CLIENT] Warning: Failed to install college LAN WFP policy.");
                        None
                    }
                }
                Err(e) => {
                    eprintln!("[CITADEL CLIENT] Warning: Failed to open WFP engine: {:?}", e);
                    None
                }
            }
        } else {
            if is_local_test {
                eprintln!("[CITADEL CLIENT] Safe network mode: Public internet preserved during local test.");
            } else if !enforce_network {
                eprintln!("[CITADEL CLIENT] Network enforcement disabled (pass --production or set CITADEL_ENFORCE_NETWORK=1 to engage).");
            } else {
                eprintln!("[CITADEL CLIENT] Non-elevated: WFP kernel network firewall skipped.");
            }
            None
        };

        // === 7. If elevated, suppress Windows Explorer shell ===
        let kill_explorer = is_production
            || std::env::var("CITADEL_KILL_EXPLORER").map(|v| v == "1").unwrap_or(false);

        let explorer_lock = if is_elevated() && kill_explorer && !is_local_test {
            Some(ExplorerLock::acquire())
        } else {
            eprintln!("[CITADEL CLIENT] Safe desktop mode: Windows Explorer preserved. Taskbar suppression active.");
            None
        };

        // === 8. Isolated secure desktop only if explicitly requested AND elevated ===
        let secure_desktop = if use_isolated_desktop && is_elevated() {
            match SecureDesktop::create() {
                Ok(sd) => Some(sd),
                Err(e) => {
                    eprintln!("[CITADEL CLIENT] Warning: Isolated Secure Desktop creation failed: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(ClientLockdownGuard {
            _hotkey_handle: hotkey_handle,
            _secure_desktop: secure_desktop,
            _explorer_lock: explorer_lock,
            _taskbar_lock: taskbar_lock,
            _touchpad_lock: touchpad_lock,
            _foreground_lock: None, // Started in launch_browser after window exists
            _clipboard_guard: clipboard_guard,
            _process_watchdog: None, // Started in launch_browser
            _wfp_engine: wfp_engine,
            _registry_lock: registry_lock,
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

        // If isolated desktop was used, switch physical display to it AFTER browser is verified alive
        if let Some(ref mut sd) = self._secure_desktop {
            sd.switch_to_secure()
                .map_err(|e| format!("Failed to switch physical display to Secure Desktop: {}", e))?;
            if let Ok(hotkey_handle) = install_hotkey_lock_with_desktop(Some(sd.handle())) {
                self._hotkey_handle = Some(hotkey_handle);
            }
        }

        // Start continuous foreground window lock
        self._foreground_lock = Some(ForegroundLock::start());

        // Start process watchdog for forbidden cheat tools
        self._process_watchdog = Some(ProcessWatchdog::start(self.violations.clone()));

        // Start background anti-cheat sensor thread (M2 loopback, M4 capture-exclusion, M5 injection)
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
