//! CITADEL Guard Anti-Cheat Sensors: M2 (Loopback), M4 (Capture Exclusion), M5 (Input Injection)
//!
//! - Module M2: Rogue Loopback Listener Scanner (offline LLMs: Ollama, LM Studio, llama.cpp)
//! - Module M4: Capture-Exclusion Window Detector (GetWindowDisplayAffinity & WDA_EXCLUDEFROMCAPTURE)
//! - Module M5: Synthetic Keystroke Injection Hook (WH_KEYBOARD_LL & LLKHF_INJECTED)

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Mutex};
use std::thread::{self, JoinHandle};

use windows::Win32::Foundation::{BOOL, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::NetworkManagement::IpHelper::*;
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};
use windows::Win32::System::Threading::{GetCurrentProcessId, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, EnumWindows, GetMessageW, GetWindowDisplayAffinity,
    GetWindowTextW, GetWindowThreadProcessId, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_QUIT,
};

use super::get_iso8601_timestamp;

// ============================================================================
// Module M2: Loopback Listener Scanner
// ============================================================================

/// Representation of an active listening socket bound to loopback
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListeningSocket {
    pub ip: IpAddr,
    pub port: u16,
    pub pid: u32,
}

/// Structured security violation event for network listeners
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViolationEvent {
    pub reason: &'static str,
    pub port: u16,
    pub pid: u32,
    pub timestamp: String,
}

impl ViolationEvent {
    pub fn to_log_line(&self) -> String {
        format!(
            "VIOLATION {} port={} pid={} {}",
            self.reason, self.port, self.pid, self.timestamp
        )
    }
}

/// Evaluates detected loopback listeners against a permitted port allow-list.
pub fn check_listener_violations(
    listeners: &[ListeningSocket],
    allowed_ports: &[u16],
) -> Vec<ViolationEvent> {
    let mut violations = Vec::new();
    let now = get_iso8601_timestamp();

    for l in listeners {
        if !allowed_ports.contains(&l.port) {
            violations.push(ViolationEvent {
                reason: "unexpected_loopback_listener",
                port: l.port,
                pid: l.pid,
                timestamp: now.clone(),
            });
        }
    }

    violations
}

/// Scans both IPv4 and IPv6 kernel TCP tables for any listening sockets bound to loopback.
pub fn scan_loopback_listeners() -> Result<Vec<ListeningSocket>, u32> {
    let mut listeners = Vec::new();
    listeners.extend(scan_ipv4_loopback_listeners()?);
    listeners.extend(scan_ipv6_loopback_listeners()?);
    Ok(listeners)
}

fn scan_ipv4_loopback_listeners() -> Result<Vec<ListeningSocket>, u32> {
    let mut size: u32 = 0;
    unsafe {
        let _ = GetExtendedTcpTable(
            None,
            &mut size,
            false,
            AF_INET.0 as u32,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        );
    }

    if size == 0 {
        return Ok(Vec::new());
    }

    let mut buf: Vec<u8> = vec![0u8; (size + 512) as usize];
    let mut actual_size = buf.len() as u32;

    let status = unsafe {
        GetExtendedTcpTable(
            Some(buf.as_mut_ptr() as *mut _),
            &mut actual_size,
            false,
            AF_INET.0 as u32,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };

    if status != 0 {
        return Err(status);
    }

    let mut listeners = Vec::new();
    let table = unsafe { &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID) };
    if table.dwNumEntries > 0 {
        let rows = unsafe {
            std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize)
        };
        for row in rows {
            let ip = Ipv4Addr::from(row.dwLocalAddr.to_ne_bytes());
            if ip.is_loopback() || ip.is_unspecified() {
                let port = u16::from_be(row.dwLocalPort as u16);
                listeners.push(ListeningSocket {
                    ip: IpAddr::V4(ip),
                    port,
                    pid: row.dwOwningPid,
                });
            }
        }
    }

    Ok(listeners)
}

fn scan_ipv6_loopback_listeners() -> Result<Vec<ListeningSocket>, u32> {
    let mut size: u32 = 0;
    unsafe {
        let _ = GetExtendedTcpTable(
            None,
            &mut size,
            false,
            AF_INET6.0 as u32,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        );
    }

    if size == 0 {
        return Ok(Vec::new());
    }

    let mut buf: Vec<u8> = vec![0u8; (size + 512) as usize];
    let mut actual_size = buf.len() as u32;

    let status = unsafe {
        GetExtendedTcpTable(
            Some(buf.as_mut_ptr() as *mut _),
            &mut actual_size,
            false,
            AF_INET6.0 as u32,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };

    if status != 0 {
        return Err(status);
    }

    let mut listeners = Vec::new();
    let table = unsafe { &*(buf.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID) };
    if table.dwNumEntries > 0 {
        let rows = unsafe {
            std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize)
        };
        for row in rows {
            let ip = Ipv6Addr::from(row.ucLocalAddr);
            if ip.is_loopback() || ip.is_unspecified() {
                let port = u16::from_be(row.dwLocalPort as u16);
                listeners.push(ListeningSocket {
                    ip: IpAddr::V6(ip),
                    port,
                    pid: row.dwOwningPid,
                });
            }
        }
    }

    Ok(listeners)
}

// ============================================================================
// Module M4: Capture-Exclusion Window Detector
// ============================================================================

/// Structured security violation event for capture-excluded windows
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcludedWindowViolation {
    pub pid: u32,
    pub title: String,
    pub affinity: u32,
    pub timestamp: String,
}

impl ExcludedWindowViolation {
    pub fn to_log_line(&self) -> String {
        format!(
            "VIOLATION capture_exclusion pid={} affinity={:#x} title=\"{}\" {}",
            self.pid, self.affinity, self.title, self.timestamp
        )
    }
}

/// Pure evaluation function: checks if a window affinity value indicates capture exclusion.
/// WDA_EXCLUDEFROMCAPTURE is 0x11 (17). Per Senior Review (doc 17 §P0-T5.3), we use a bitwise
/// check `(affinity & 0x11) == 0x11` to handle future composite Windows flags gracefully.
pub fn is_capture_excluded_affinity(affinity: u32) -> bool {
    (affinity & 0x11) == 0x11
}

/// Enumerates all top-level windows in the system and returns any non-exempt windows
/// configured with capture exclusion (WDA_EXCLUDEFROMCAPTURE).
pub fn scan_capture_exclusion_windows() -> Vec<ExcludedWindowViolation> {
    let own_pid = unsafe { GetCurrentProcessId() };

    struct EnumContext {
        own_pid: u32,
        violations: Vec<ExcludedWindowViolation>,
    }

    let mut ctx = EnumContext {
        own_pid,
        violations: Vec::new(),
    };

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam.0 as *mut EnumContext);
        let mut pid: u32 = 0;
        let _ = GetWindowThreadProcessId(hwnd, Some(&mut pid));

        // Skip windows belonging to CITADEL Guard itself
        if pid != 0 && pid != ctx.own_pid {
            let mut affinity: u32 = 0;
            if GetWindowDisplayAffinity(hwnd, &mut affinity).is_ok() {
                if is_capture_excluded_affinity(affinity) {
                    let mut title_buf = [0u16; 256];
                    let len = GetWindowTextW(hwnd, &mut title_buf);
                    let title = if len > 0 {
                        String::from_utf16_lossy(&title_buf[..len as usize])
                    } else {
                        String::from("<untitled>")
                    };

                    ctx.violations.push(ExcludedWindowViolation {
                        pid,
                        title,
                        affinity,
                        timestamp: get_iso8601_timestamp(),
                    });
                }
            }
        }

        BOOL(1)
    }

    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut ctx as *mut _ as isize));
    }

    ctx.violations
}

// ============================================================================
// Module M5: Synthetic Keystroke Injection Hook
// ============================================================================

/// Structured security violation event for synthetic keystroke injection
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeystrokeViolation {
    pub vk_code: u32,
    pub timestamp: String,
}

impl KeystrokeViolation {
    pub fn to_log_line(&self) -> String {
        format!(
            "VIOLATION input_injection vk_code={} {}",
            self.vk_code, self.timestamp
        )
    }
}

/// Pure evaluation function: checks if a KBDLLHOOKSTRUCT flags field has LLKHF_INJECTED (0x10) set.
pub fn is_injected_keystroke_flag(flags: u32) -> bool {
    (flags & 0x10) != 0
}

static INJECTED_KEYSTROKE_COUNT: AtomicUsize = AtomicUsize::new(0);
static VIOLATION_SINK: Mutex<Option<Vec<KeystrokeViolation>>> = Mutex::new(None);

/// Returns the total number of injected keystrokes detected since service start.
pub fn get_injected_keystroke_count() -> usize {
    INJECTED_KEYSTROKE_COUNT.load(Ordering::SeqCst)
}

/// Drains and returns all queued keystroke injection violations.
pub fn take_injected_keystroke_violations() -> Vec<KeystrokeViolation> {
    let mut sink = VIOLATION_SINK.lock().unwrap();
    if let Some(ref mut vec) = *sink {
        std::mem::take(vec)
    } else {
        Vec::new()
    }
}

unsafe extern "system" fn keyboard_hook_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let kbd = *(lparam.0 as *const KBDLLHOOKSTRUCT);
        if is_injected_keystroke_flag(kbd.flags.0) {
            INJECTED_KEYSTROKE_COUNT.fetch_add(1, Ordering::SeqCst);
            let violation = KeystrokeViolation {
                vk_code: kbd.vkCode,
                timestamp: get_iso8601_timestamp(),
            };
            if let Ok(mut sink) = VIOLATION_SINK.lock() {
                if let Some(ref mut list) = *sink {
                    list.push(violation);
                }
            }
        }
    }
    CallNextHookEx(HHOOK(std::ptr::null_mut()), code, wparam, lparam)
}

/// RAII handle to the low-level keyboard hook background thread.
/// When dropped, sends WM_QUIT to the thread's message queue and unhooks cleanly.
pub struct KeyboardHookHandle {
    thread_id: u32,
    join_handle: Option<JoinHandle<()>>,
}

impl KeyboardHookHandle {
    pub fn stop(mut self) {
        self.cleanup();
    }

    fn cleanup(&mut self) {
        if self.thread_id != 0 {
            unsafe {
                let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
            self.thread_id = 0;
        }
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for KeyboardHookHandle {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Installs the system-wide WH_KEYBOARD_LL hook on a dedicated background thread with
/// its own Windows message pump.
pub fn install_keyboard_hook() -> Result<KeyboardHookHandle, String> {
    // Initialize violation sink
    {
        let mut sink = VIOLATION_SINK.lock().unwrap();
        if sink.is_none() {
            *sink = Some(Vec::new());
        }
    }

    let (tx, rx) = mpsc::channel::<Result<u32, String>>();

    let join_handle = thread::spawn(move || {
        let thread_id = unsafe { GetCurrentThreadId() };
        let hhook = unsafe {
            SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_hook_proc),
                None,
                0,
            )
        };

        let hhook = match hhook {
            Ok(h) => {
                let _ = tx.send(Ok(thread_id));
                h
            }
            Err(e) => {
                let _ = tx.send(Err(format!("SetWindowsHookExW failed: {:?}", e)));
                return;
            }
        };

        // Standard Win32 Message Loop required for low-level hooks
        let mut msg = MSG::default();
        while unsafe { GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0).as_bool() } {
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        // Clean unhook on thread exit
        unsafe {
            let _ = UnhookWindowsHookEx(hhook);
        }
    });

    let thread_id = rx
        .recv()
        .map_err(|e| format!("Failed to receive hook thread init: {}", e))??;

    Ok(KeyboardHookHandle {
        thread_id,
        join_handle: Some(join_handle),
    })
}
