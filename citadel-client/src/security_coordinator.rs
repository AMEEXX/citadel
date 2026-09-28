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

use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::io::{Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
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


pub struct BluetoothLock {
    was_active: bool,
}

impl BluetoothLock {
    pub fn acquire() -> Self {
        eprintln!("[CITADEL CLIENT] Disabling Bluetooth service for exam security...");
        let _ = std::process::Command::new("net")
            .args(["stop", "bthserv", "/y"])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output();

        BluetoothLock { was_active: true }
    }

    pub fn restore(&self) {
        if self.was_active {
            eprintln!("[CITADEL CLIENT] Restoring Bluetooth service...");
            let _ = std::process::Command::new("sc")
                .args(["config", "bthserv", "start=", "auto"])
                .creation_flags(0x08000000)
                .output();
            let _ = std::process::Command::new("net")
                .args(["start", "bthserv"])
                .creation_flags(0x08000000)
                .output();
        }
    }
}

impl Drop for BluetoothLock {
    fn drop(&mut self) {
        self.restore();
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
    _bluetooth_lock: Option<BluetoothLock>,

    stop_signal: Arc<AtomicBool>,
    sensor_thread: Option<JoinHandle<()>>,
    violations: Arc<Mutex<Vec<String>>>,
    server_ip: Ipv4Addr,
    server_port: u16,
    pub auth_token: Option<String>,
    pub is_production: bool,
    /// Tracks whether restore_all() has already been called to prevent double-restore
    restored: bool,
}


pub fn perform_client_handshake(server_ip: Ipv4Addr, server_port: u16, is_production: bool) -> Option<String> {
    let addr = SocketAddr::from((server_ip, server_port));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(1500)).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_millis(1500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(1500)));

    let elevated = is_elevated();
    let body = format!(
        r#"{{"client_version":"0.2.0","machine_guid":null,"mode":"{}","is_elevated":{},"elevation_proof":{}}}"#,
        if is_production { "production" } else { "testing" },
        elevated,
        if elevated {
            format!("\"citadel-elevated-{:x}\"", (std::process::id() as u64) ^ 0x0ace11ed)
        } else {
            "null".to_string()
        }
    );

    let req = format!(
        "POST /api/v1/client/handshake HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        server_ip, server_port, body.len(), body
    );

    let _ = stream.write_all(req.as_bytes());

    let mut resp = Vec::new();
    let mut buf = [0u8; 1024];
    while let Ok(n) = stream.read(&mut buf) {
        if n == 0 { break; }
        resp.extend_from_slice(&buf[..n]);
    }

    let resp_str = String::from_utf8_lossy(&resp);
    if let Some(pos) = resp_str.find("\"session_token\":\"") {
        let after = &resp_str[pos + 17..];
        if let Some(end) = after.find('"') {
            let token = &after[..end];
            eprintln!("[CITADEL CLIENT] Client handshake successful! Session token: {}", token);
            return Some(token.to_string());
        }
    }

    eprintln!("[CITADEL CLIENT] Handshake did not return session token.");
    None
}

impl ClientLockdownGuard {
    /// Prepares the client lockdown in the background without affecting the user's display.
    pub fn new(server_ip: Ipv4Addr, server_port: u16) -> Result<Self, String> {
        Self::new_with_mode(server_ip, server_port, false, false)
    }

    pub fn new_with_mode(server_ip: Ipv4Addr, server_port: u16, use_isolated_desktop: bool, is_production: bool) -> Result<Self, String> {
        install_crash_safety();

        // === ZERO-FALLBACK MANDATORY ELEVATION ENFORCEMENT ===
        // The Citadel Client MUST ONLY run with elevated Administrator privileges.
        // Degrading into an unprivileged "less control" mode is strictly prohibited.
        if !is_elevated() {
            let err_msg = "MANDATORY SECURITY ENFORCEMENT: Citadel Client requires Administrator privileges.                            Running in an unprivileged or degraded 'less control' mode is strictly prohibited by Citadel Security Policy.";
            eprintln!("[CITADEL CLIENT FATAL] {}", err_msg);
            return Err(err_msg.to_string());
        }

        let auth_token = perform_client_handshake(server_ip, server_port, is_production);

        let violations = Arc::new(Mutex::new(Vec::new()));
        let stop_signal = Arc::new(AtomicBool::new(false));

        // === 1. Host Desktop Protection: Host registry policies are strictly NEVER touched ===
        let registry_lock = None;

        // Bluetooth hardware & service suppression (guaranteed elevated)
        let bluetooth_lock = Some(BluetoothLock::acquire());
        eprintln!("[CITADEL CLIENT] Host desktop protection active: Primary desktop registry policies preserved.");

        // Set active mode in hotkey_lock module
        crate::hotkey_lock::set_production_mode(is_production);

        // === 2. ALWAYS install system-wide low-level keyboard hook immediately ===
        let hotkey_handle = match install_hotkey_lock() {
            Ok(hk) => {
                eprintln!("[CITADEL CLIENT] KEYBOARD HOOK ACTIVE: Alt+Tab, Win keys, system combos blocked.");
                Some(hk)
            }
            Err(e) => {
                if is_production {
                    let err_msg = format!("MANDATORY SECURITY ENFORCEMENT: Failed to install low-level keyboard suppression hook in Production Mode: {}. Aborting startup.", e);
                    eprintln!("[CITADEL CLIENT FATAL] {}", err_msg);
                    return Err(err_msg);
                } else {
                    eprintln!("[CITADEL CLIENT] Warning: Keyboard hook failed: {}", e);
                    None
                }
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

        let wfp_engine = if enforce_network && !is_local_test {
            match WfpEngine::open_dynamic() {
                Ok(mut engine) => {
                    if engine.install_college_lan_policy(server_ip, server_port).is_ok() {
                        eprintln!(
                            "[CITADEL CLIENT] HARDWARE NETWORK LOCK ACTIVE: Permitted server: {}:{}",
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
            } else {
                eprintln!("[CITADEL CLIENT] Network enforcement disabled.");
            }
            None
        };

        // === 7. Suppress Windows Explorer shell if in production ===
        let kill_explorer = is_production
            || std::env::var("CITADEL_KILL_EXPLORER").map(|v| v == "1").unwrap_or(false);

        let explorer_lock = if kill_explorer && !is_local_test {
            Some(ExplorerLock::acquire())
        } else {
            eprintln!("[CITADEL CLIENT] Safe desktop mode: Windows Explorer preserved. Taskbar suppression active.");
            None
        };

        // === 8. Isolated secure desktop only if explicitly requested ===
        let secure_desktop = if use_isolated_desktop {
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
            _bluetooth_lock: bluetooth_lock,
            stop_signal,
            sensor_thread: None,
            violations,
            server_ip,
            server_port,
            auth_token,
            is_production,
            restored: false,
        })
    }

    pub fn server_endpoint(&self) -> String {
        if let Some(ref token) = self.auth_token {
            format!("http://{}:{}/exam?auth_token={}", self.server_ip, self.server_port, token)
        } else {
            format!("http://{}:{}/?token=citadel-secured-session", self.server_ip, self.server_port)
        }
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

        // Start continuous foreground window lock ONLY in Production Mode and ONLY for our browser PID
        if self.is_production {
            self._foreground_lock = Some(ForegroundLock::start(kiosk_child.known_pids.clone()));
        } else {
            eprintln!("[CITADEL CLIENT] TESTING MODE: ForegroundLock disabled to preserve normal window switching.");
        }

        // Start process watchdog for forbidden cheat tools
        self._process_watchdog = Some(ProcessWatchdog::start(self.violations.clone(), kiosk_child.known_pids.clone(), self.is_production));

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

    /// Explicitly tear down ALL lockdown components in guaranteed order.
    /// Called by both Drop and End Exam to ensure full restoration.
    /// This is idempotent — calling it multiple times is safe.
    pub fn restore_all(&mut self) {
        if self.restored {
            eprintln!("[CITADEL CLIENT] restore_all() already executed, skipping.");
            return;
        }
        self.restored = true;

        eprintln!("[CITADEL CLIENT] === FULL SYSTEM RESTORATION INITIATED ===");

        // 1. Signal all background threads to stop FIRST
        self.stop_signal.store(true, Ordering::SeqCst);

        // 2. Stop sensor thread
        if let Some(thread) = self.sensor_thread.take() {
            let _ = thread.join();
            eprintln!("[CITADEL CLIENT] [RESTORE] Sensor thread stopped.");
        }

        // 3. Stop process watchdog
        if let Some(wd) = self._process_watchdog.take() {
            drop(wd);
            eprintln!("[CITADEL CLIENT] [RESTORE] Process watchdog stopped.");
        }

        // 4. Stop foreground lock
        if let Some(fl) = self._foreground_lock.take() {
            drop(fl);
            eprintln!("[CITADEL CLIENT] [RESTORE] Foreground lock released.");
        }

        // 5. Stop clipboard guard
        if let Some(cg) = self._clipboard_guard.take() {
            drop(cg);
            eprintln!("[CITADEL CLIENT] [RESTORE] Clipboard guard stopped.");
        }

        // 6. Release touchpad lock
        if let Some(tp) = self._touchpad_lock.take() {
            drop(tp);
            eprintln!("[CITADEL CLIENT] [RESTORE] Touchpad lock released.");
        }

        // 7. Release taskbar lock (restore taskbars)
        if let Some(tb) = self._taskbar_lock.take() {
            drop(tb);
            eprintln!("[CITADEL CLIENT] [RESTORE] Taskbar restored.");
        }

        // 8. CRITICAL: Unhook keyboard hook — this is what restores Win key, Alt+Tab
        if let Some(hk) = self._hotkey_handle.take() {
            hk.stop();
            eprintln!("[CITADEL CLIENT] [RESTORE] Keyboard hook uninstalled - Win key, Alt+Tab RESTORED.");
        }

        // 9. Release WFP engine (remove kernel firewall rules)
        if let Some(wfp) = self._wfp_engine.take() {
            drop(wfp);
            eprintln!("[CITADEL CLIENT] [RESTORE] WFP kernel firewall rules removed - Internet RESTORED.");
        }

        // 10. Restore explorer lock
        if let Some(el) = self._explorer_lock.take() {
            drop(el);
            eprintln!("[CITADEL CLIENT] [RESTORE] Explorer shell relaunched.");
        }

        // 11. Switch back from secure desktop
        if let Some(sd) = self._secure_desktop.take() {
            drop(sd);
            eprintln!("[CITADEL CLIENT] [RESTORE] Switched back to Default desktop.");
        }

        // 12. Restore registry policies
        if let Some(mut rl) = self._registry_lock.take() {
            rl.restore();
            eprintln!("[CITADEL CLIENT] [RESTORE] Registry policies restored.");
        }

        // 13. Restore Bluetooth — call restore() then forget to avoid double-drop
        if let Some(bl) = self._bluetooth_lock.take() {
            bl.restore();
            std::mem::forget(bl);
            eprintln!("[CITADEL CLIENT] [RESTORE] Bluetooth service restored.");
        }

        // 14. Restore WLAN service (always attempt)
        let _ = std::process::Command::new("sc")
            .args(["config", "WlanSvc", "start=", "auto"])
            .creation_flags(0x08000000)
            .output();
        let _ = std::process::Command::new("net")
            .args(["start", "WlanSvc"])
            .creation_flags(0x08000000)
            .output();
        eprintln!("[CITADEL CLIENT] [RESTORE] WLAN service restored.");

        // 15. Run emergency_restore_system as final safety net (cleans registry, restores desktop)
        crate::crash_handler::emergency_restore_system();

        eprintln!("[CITADEL CLIENT] === FULL SYSTEM RESTORATION COMPLETE ===");
        eprintln!("[CITADEL CLIENT] All restrictions removed. System returned to normal state.");
    }
}

impl Drop for ClientLockdownGuard {
    fn drop(&mut self) {
        eprintln!("[CITADEL CLIENT] Guard dropping — initiating full system restoration...");
        self.restore_all();
    }
}
