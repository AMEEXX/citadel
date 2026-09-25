//! CITADEL Client Security Coordinator
//!
//! Orchestrates the client-side lockdown lifecycle:
//! 1. WFP dynamic network isolation (zero internet, college server only)
//! 2. Low-level hotkey suppression (blocks Alt-Tab, Win, Alt-F4)
//! 3. Background anti-cheat sensors (M2 loopback, M4 capture-exclusion, M5 injection)
//! 4. Safe RAII cleanup upon exit

use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use guard_net::WfpEngine;
use guard_svc::llm_detect;

use crate::hotkey_lock::{install_hotkey_lock, HotkeyLockHandle};

pub struct ClientLockdownGuard {
    _wfp_engine: Option<WfpEngine>,
    _hotkey_handle: Option<HotkeyLockHandle>,
    stop_signal: Arc<AtomicBool>,
    sensor_thread: Option<JoinHandle<()>>,
    violations: Arc<Mutex<Vec<String>>>,
    server_ip: Ipv4Addr,
    server_port: u16,
}

impl ClientLockdownGuard {
    /// Initializes client-side lockdown for the target exam server.
    /// If administrative privilege is missing, WFP may fail cleanly with a descriptive error.
    pub fn new(server_ip: Ipv4Addr, server_port: u16) -> Result<Self, String> {
        let violations = Arc::new(Mutex::new(Vec::new()));
        let stop_signal = Arc::new(AtomicBool::new(false));

        // 1. Install WFP Zero-Internet Filter
        let wfp_engine = match WfpEngine::open_dynamic() {
            Ok(mut engine) => {
                match engine.install_college_lan_policy(server_ip, server_port) {
                    Ok(()) => {
                        eprintln!(
                            "[CITADEL CLIENT] Network lockdown ACTIVE: only {}:{} permitted.",
                            server_ip, server_port
                        );
                        Some(engine)
                    }
                    Err(e) => {
                        eprintln!(
                            "[CITADEL CLIENT] WARNING: WFP policy failed ({:?}). Running in non-WFP mode.",
                            e
                        );
                        None
                    }
                }
            }
            Err(e) => {
                eprintln!(
                    "[CITADEL CLIENT] WARNING: WFP engine open failed ({:?}). (Requires Administrator)",
                    e
                );
                None
            }
        };

        // 2. Install Hotkey Suppression Hook
        let hotkey_handle = match install_hotkey_lock() {
            Ok(handle) => {
                eprintln!("[CITADEL CLIENT] Hotkey suppression ACTIVE: Alt-Tab and Win keys locked.");
                Some(handle)
            }
            Err(e) => {
                eprintln!("[CITADEL CLIENT] WARNING: Hotkey lock failed: {}", e);
                None
            }
        };

        // 3. Start background anti-cheat watchdog
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
            stop_signal,
            sensor_thread: Some(sensor_thread),
            violations,
            server_ip,
            server_port,
        })
    }

    pub fn server_endpoint(&self) -> String {
        format!("http://{}:{}", self.server_ip, self.server_port)
    }

    pub fn get_violations(&self) -> Vec<String> {
        self.violations.lock().unwrap().clone()
    }
}

impl Drop for ClientLockdownGuard {
    fn drop(&mut self) {
        eprintln!("[CITADEL CLIENT] Releasing client lockdown and restoring normal network...");
        self.stop_signal.store(true, Ordering::Relaxed);
        if let Some(thread) = self.sensor_thread.take() {
            let _ = thread.join();
        }
        // Dropping _hotkey_handle and _wfp_engine automatically triggers their Drop impls
    }
}
