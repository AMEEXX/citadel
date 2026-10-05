//! CITADEL Client Kiosk Window Manager & Shell Hardening
//!
//! Enforces:
//! 1. Full-screen isolated kiosk browser window launch with Chromium security flags
//!    directly targeted to the Win32 Secure Desktop plane (lpDesktop).
//! 2. Persistent Taskbar suppression (SW_HIDE + EnableWindow(false) + HWND_BOTTOM)
//! 3. Precision Touchpad gesture suppression (disables 3-finger and 4-finger swipes via Registry)
//! 4. Continuous Foreground window dominance (locks kiosk window to HWND_TOPMOST)
//! 5. System clipboard wiper (prevents external copy/paste data leakage)
//! 6. Active process watchdog (detects and terminates blacklisted cheat processes)

use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{BOOL, CloseHandle, HANDLE, HWND, LPARAM, RECT, WPARAM};
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_DWORD, REG_SZ,
    REG_VALUE_TYPE,
};
use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};
use windows::Win32::System::Threading::{
    CreateProcessW, GetExitCodeProcess, OpenProcess, TerminateProcess, CREATE_NEW_PROCESS_GROUP,
    PROCESS_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, STARTF_USESHOWWINDOW, STARTUPINFOW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, FindWindowW, GetForegroundWindow, GetSystemMetrics,
    GetWindowRect, GetWindowThreadProcessId, SetForegroundWindow, SetWindowPos, ShowWindow,
    HWND_BOTTOM, HWND_BROADCAST, HWND_NOTOPMOST, HWND_TOPMOST, SM_CXSCREEN, SM_CYSCREEN,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    SW_HIDE, SW_MAXIMIZE, SW_SHOW, SendMessageTimeoutW, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
};

// ============================================================================
// 1. Taskbar and Shell Lock
// ============================================================================

/// RAII Guard that hides and disables the Windows Shell Taskbar, Start button,
/// and secondary monitor taskbars, continuously enforcing this state every 200ms.
pub struct TaskbarLock {
    stop_signal: Arc<AtomicBool>,
    watchdog_thread: Option<JoinHandle<()>>,
}

impl TaskbarLock {
    pub fn acquire() -> Self {
        let stop_signal = Arc::new(AtomicBool::new(false));

        // Initial hide and disable
        Self::apply_taskbar_state(false);

        // Continuous enforcement loop
        let stop_clone = stop_signal.clone();
        let watchdog_thread = thread::spawn(move || {
            while !stop_clone.load(Ordering::Relaxed) {
                Self::apply_taskbar_state(false);
                thread::sleep(Duration::from_millis(200));
            }
        });

        TaskbarLock {
            stop_signal,
            watchdog_thread: Some(watchdog_thread),
        }
    }

    fn apply_taskbar_state(enable: bool) {
        unsafe {
            // Primary taskbar
            if let Ok(taskbar) = FindWindowW(w!("Shell_TrayWnd"), None) {
                let _ = EnableWindow(taskbar, BOOL(if enable { 1 } else { 0 }));
                let _ = ShowWindow(taskbar, if enable { SW_SHOW } else { SW_HIDE });
                if !enable {
                    let _ = SetWindowPos(
                        taskbar,
                        HWND_BOTTOM,
                        0, 0, 0, 0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }
            }

            // Secondary monitor taskbars
            if let Ok(sec_taskbar) = FindWindowW(w!("Shell_SecondaryTrayWnd"), None) {
                let _ = EnableWindow(sec_taskbar, BOOL(if enable { 1 } else { 0 }));
                let _ = ShowWindow(sec_taskbar, if enable { SW_SHOW } else { SW_HIDE });
                if !enable {
                    let _ = SetWindowPos(
                        sec_taskbar,
                        HWND_BOTTOM,
                        0, 0, 0, 0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }
            }
        }
    }
}

impl Drop for TaskbarLock {
    fn drop(&mut self) {
        self.stop_signal.store(true, Ordering::Relaxed);
        if let Some(handle) = self.watchdog_thread.take() {
            let _ = handle.join();
        }
        // Restore taskbar and start menu to full functionality
        Self::apply_taskbar_state(true);
    }
}

// ============================================================================
// 2. Touchpad Gesture Protection, Snapshot & Lockdown
// ============================================================================

pub const PRECISION_TOUCHPAD_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\PrecisionTouchPad";
pub const PRECISION_TOUCHPAD_GESTURES_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\PrecisionTouchPad\Gestures";
pub const CITADEL_GESTURE_BACKUP_SUBKEY: &str = r"Software\Citadel\GestureBackup";

pub const TOUCHPAD_DWORD_KEYS: &[&str] = &[
    "ThreeFingerSlideEnabled",
    "ThreeFingerTapEnabled",
    "FourFingerSlideEnabled",
    "FourFingerTapEnabled",
    "ThreeFingerSlideUp",
    "ThreeFingerSlideDown",
    "ThreeFingerSlideLeft",
    "ThreeFingerSlideRight",
    "ThreeFingerTap",
    "FourFingerSlideUp",
    "FourFingerSlideDown",
    "FourFingerSlideLeft",
    "FourFingerSlideRight",
    "FourFingerTap",
    "ThreeFingerDownEnabled",
    "FourFingerDownEnabled",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TouchpadBackup {
    pub values: HashMap<String, Option<u32>>,
    pub gestures_sub_values: HashMap<String, Option<u32>>,
    pub timestamp: u64,
}

impl TouchpadBackup {
    pub fn read_current_state() -> Self {
        let mut values = HashMap::new();
        let subkey_w = to_wide_str(PRECISION_TOUCHPAD_SUBKEY);
        unsafe {
            let mut hkey = HKEY::default();
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey_w.as_ptr()),
                0,
                KEY_READ,
                &mut hkey,
            ).is_ok() {
                for &k in TOUCHPAD_DWORD_KEYS {
                    let kw = to_wide_str(k);
                    let mut data = [0u8; 4];
                    let mut data_len = 4u32;
                    let mut rtype = REG_VALUE_TYPE(0);
                    if RegQueryValueExW(
                        hkey,
                        PCWSTR(kw.as_ptr()),
                        None,
                        Some(&mut rtype),
                        Some(data.as_mut_ptr()),
                        Some(&mut data_len),
                    ).is_ok() && rtype == REG_DWORD && data_len == 4 {
                        values.insert(k.to_string(), Some(u32::from_le_bytes(data)));
                    } else {
                        values.insert(k.to_string(), None);
                    }
                }
                let _ = RegCloseKey(hkey);
            } else {
                for &k in TOUCHPAD_DWORD_KEYS {
                    values.insert(k.to_string(), None);
                }
            }
        }

        let mut gestures_sub_values = HashMap::new();
        let gestures_w = to_wide_str(PRECISION_TOUCHPAD_GESTURES_SUBKEY);
        unsafe {
            let mut hkey = HKEY::default();
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(gestures_w.as_ptr()),
                0,
                KEY_READ,
                &mut hkey,
            ).is_ok() {
                for &k in TOUCHPAD_DWORD_KEYS {
                    let kw = to_wide_str(k);
                    let mut data = [0u8; 4];
                    let mut data_len = 4u32;
                    let mut rtype = REG_VALUE_TYPE(0);
                    if RegQueryValueExW(
                        hkey,
                        PCWSTR(kw.as_ptr()),
                        None,
                        Some(&mut rtype),
                        Some(data.as_mut_ptr()),
                        Some(&mut data_len),
                    ).is_ok() && rtype == REG_DWORD && data_len == 4 {
                        gestures_sub_values.insert(k.to_string(), Some(u32::from_le_bytes(data)));
                    } else {
                        gestures_sub_values.insert(k.to_string(), None);
                    }
                }
                let _ = RegCloseKey(hkey);
            }
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            values,
            gestures_sub_values,
            timestamp,
        }
    }

    pub fn persist(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            // 1. %ProgramData%\Citadel\state\pre_exam_snapshot.json
            let program_data = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".to_string());
            let pd_dir = std::path::Path::new(&program_data).join("Citadel").join("state");
            let _ = std::fs::create_dir_all(&pd_dir);
            let pd_file = pd_dir.join("pre_exam_snapshot.json");
            let _ = std::fs::write(&pd_file, &json);

            // 2. %TEMP%\citadel_gesture_backup.json
            let temp_dir = std::env::var("TEMP").unwrap_or_else(|_| r"C:\Windows\Temp".to_string());
            let temp_file = std::path::Path::new(&temp_dir).join("citadel_gesture_backup.json");
            let _ = std::fs::write(&temp_file, &json);

            // 3. HKCU\Software\Citadel\GestureBackup
            let reg_subkey = to_wide_str(CITADEL_GESTURE_BACKUP_SUBKEY);
            unsafe {
                let mut hkey = HKEY::default();
                if RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    PCWSTR(reg_subkey.as_ptr()),
                    0,
                    None,
                    windows::Win32::System::Registry::REG_OPEN_CREATE_OPTIONS(0),
                    KEY_WRITE,
                    None,
                    &mut hkey,
                    None,
                ).is_ok() {
                    let json_w = to_wide_str(&json);
                    let val_name_w = to_wide_str("SnapshotJson");
                    let bytes = std::slice::from_raw_parts(
                        json_w.as_ptr() as *const u8,
                        json_w.len() * 2,
                    );
                    let _ = RegSetValueExW(
                        hkey,
                        PCWSTR(val_name_w.as_ptr()),
                        0,
                        REG_SZ,
                        Some(bytes),
                    );
                    let _ = RegCloseKey(hkey);
                }
            }
        }
    }

    pub fn load() -> Option<Self> {
        // Priority 1: ProgramData
        let program_data = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".to_string());
        let pd_file = std::path::Path::new(&program_data).join("Citadel").join("state").join("pre_exam_snapshot.json");
        if let Ok(content) = std::fs::read_to_string(&pd_file) {
            if let Ok(backup) = serde_json::from_str::<Self>(&content) {
                return Some(backup);
            }
        }

        // Priority 2: TEMP
        let temp_dir = std::env::var("TEMP").unwrap_or_else(|_| r"C:\Windows\Temp".to_string());
        let temp_file = std::path::Path::new(&temp_dir).join("citadel_gesture_backup.json");
        if let Ok(content) = std::fs::read_to_string(&temp_file) {
            if let Ok(backup) = serde_json::from_str::<Self>(&content) {
                return Some(backup);
            }
        }

        // Priority 3: Registry
        let reg_subkey = to_wide_str(CITADEL_GESTURE_BACKUP_SUBKEY);
        unsafe {
            let mut hkey = HKEY::default();
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(reg_subkey.as_ptr()),
                0,
                KEY_READ,
                &mut hkey,
            ).is_ok() {
                let val_name_w = to_wide_str("SnapshotJson");
                let mut buf = vec![0u8; 16384];
                let mut buf_len = buf.len() as u32;
                let mut rtype = REG_VALUE_TYPE(0);
                if RegQueryValueExW(
                    hkey,
                    PCWSTR(val_name_w.as_ptr()),
                    None,
                    Some(&mut rtype),
                    Some(buf.as_mut_ptr()),
                    Some(&mut buf_len),
                ).is_ok() {
                    let u16_slice = std::slice::from_raw_parts(
                        buf.as_ptr() as *const u16,
                        (buf_len as usize) / 2,
                    );
                    let s = String::from_utf16_lossy(u16_slice);
                    let trimmed = s.trim_matches(char::from(0));
                    if let Ok(backup) = serde_json::from_str::<Self>(trimmed) {
                        let _ = RegCloseKey(hkey);
                        return Some(backup);
                    }
                }
                let _ = RegCloseKey(hkey);
            }
        }

        None
    }
}

pub struct TouchpadLock {
    pub backup: Option<TouchpadBackup>,
}

fn to_wide_str(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

impl TouchpadLock {
    pub fn acquire() -> Self {
        Self::snapshot_and_disable()
    }

    /// Step 0 of Pre-flight: snapshots current touchpad state prior to any kills/locks
    pub fn snapshot_before_exam() -> TouchpadBackup {
        if let Some(existing) = TouchpadBackup::load() {
            eprintln!("[TOUCHPAD] Existing pre-exam gesture snapshot found (ts: {}). Retaining.", existing.timestamp);
            return existing;
        }

        eprintln!("[TOUCHPAD] Capturing clean pre-exam gesture snapshot...");
        let backup = TouchpadBackup::read_current_state();
        backup.persist();
        eprintln!("[TOUCHPAD] Clean pre-exam gesture snapshot persisted to ProgramData, TEMP, and HKCU.");
        backup
    }

    /// Disables touchpad multi-finger gestures during the exam session
    pub fn snapshot_and_disable() -> Self {
        let backup = Self::snapshot_before_exam();

        eprintln!("[TOUCHPAD] Disabling precision touchpad gestures for exam lockdown...");
        let zero_bytes = 0u32.to_le_bytes();

        let subkey_w = to_wide_str(PRECISION_TOUCHPAD_SUBKEY);
        unsafe {
            let mut hkey = HKEY::default();
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey_w.as_ptr()),
                0,
                KEY_WRITE,
                &mut hkey,
            ).is_ok() {
                for &val_name in TOUCHPAD_DWORD_KEYS {
                    let val_w = to_wide_str(val_name);
                    let _ = RegSetValueExW(
                        hkey,
                        PCWSTR(val_w.as_ptr()),
                        0,
                        REG_DWORD,
                        Some(&zero_bytes),
                    );
                }
                let _ = RegCloseKey(hkey);
            }

            let gestures_w = to_wide_str(PRECISION_TOUCHPAD_GESTURES_SUBKEY);
            let mut ghkey = HKEY::default();
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(gestures_w.as_ptr()),
                0,
                KEY_WRITE,
                &mut ghkey,
            ).is_ok() {
                for &val_name in TOUCHPAD_DWORD_KEYS {
                    let val_w = to_wide_str(val_name);
                    let _ = RegSetValueExW(
                        ghkey,
                        PCWSTR(val_w.as_ptr()),
                        0,
                        REG_DWORD,
                        Some(&zero_bytes),
                    );
                }
                let _ = RegCloseKey(ghkey);
            }

            // Signal shell of registry changes
            let _ = SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                WPARAM(0),
                LPARAM(subkey_w.as_ptr() as isize),
                SMTO_ABORTIFHUNG,
                1000,
                None,
            );
            SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
        }

        TouchpadLock { backup: Some(backup) }
    }

    /// Restores touchpad gestures back to their pre-exam snapshot
    pub fn restore_touchpad_gestures() {
        eprintln!("[TOUCHPAD] Restoring precision touchpad gestures from snapshot...");
        let maybe_backup = TouchpadBackup::load();

        if let Some(backup) = maybe_backup {
            let subkey_w = to_wide_str(PRECISION_TOUCHPAD_SUBKEY);
            unsafe {
                let mut hkey = HKEY::default();
                if RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    PCWSTR(subkey_w.as_ptr()),
                    0,
                    KEY_WRITE,
                    &mut hkey,
                ).is_ok() {
                    for (k, opt_val) in backup.values {
                        let kw = to_wide_str(&k);
                        if let Some(val) = opt_val {
                            let bytes = val.to_le_bytes();
                            let _ = RegSetValueExW(
                                hkey,
                                PCWSTR(kw.as_ptr()),
                                0,
                                REG_DWORD,
                                Some(&bytes),
                            );
                        } else {
                            let _ = RegDeleteValueW(hkey, PCWSTR(kw.as_ptr()));
                        }
                    }
                    let _ = RegCloseKey(hkey);
                }

                let gestures_w = to_wide_str(PRECISION_TOUCHPAD_GESTURES_SUBKEY);
                let mut ghkey = HKEY::default();
                if RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    PCWSTR(gestures_w.as_ptr()),
                    0,
                    KEY_WRITE,
                    &mut ghkey,
                ).is_ok() {
                    for (k, opt_val) in backup.gestures_sub_values {
                        let kw = to_wide_str(&k);
                        if let Some(val) = opt_val {
                            let bytes = val.to_le_bytes();
                            let _ = RegSetValueExW(
                                ghkey,
                                PCWSTR(kw.as_ptr()),
                                0,
                                REG_DWORD,
                                Some(&bytes),
                            );
                        } else {
                            let _ = RegDeleteValueW(ghkey, PCWSTR(kw.as_ptr()));
                        }
                    }
                    let _ = RegCloseKey(ghkey);
                }

                let _ = SendMessageTimeoutW(
                    HWND_BROADCAST,
                    WM_SETTINGCHANGE,
                    WPARAM(0),
                    LPARAM(subkey_w.as_ptr() as isize),
                    SMTO_ABORTIFHUNG,
                    1000,
                    None,
                );
                SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
            }
        } else {
            // Fallback: enable standard defaults
            Self::restore_system_defaults();
        }

        // Clean up persisted snapshot files
        let program_data = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".to_string());
        let pd_file = std::path::Path::new(&program_data).join("Citadel").join("state").join("pre_exam_snapshot.json");
        let _ = std::fs::remove_file(pd_file);

        let temp_dir = std::env::var("TEMP").unwrap_or_else(|_| r"C:\Windows\Temp".to_string());
        let temp_file = std::path::Path::new(&temp_dir).join("citadel_gesture_backup.json");
        let _ = std::fs::remove_file(temp_file);

        let reg_subkey = to_wide_str(CITADEL_GESTURE_BACKUP_SUBKEY);
        unsafe {
            let mut hkey = HKEY::default();
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(reg_subkey.as_ptr()),
                0,
                KEY_WRITE,
                &mut hkey,
            ).is_ok() {
                let val_name_w = to_wide_str("SnapshotJson");
                let _ = RegDeleteValueW(hkey, PCWSTR(val_name_w.as_ptr()));
                let _ = RegCloseKey(hkey);
            }
        }
    }

    pub fn restore_system_defaults() {
        let subkey_w = to_wide_str(PRECISION_TOUCHPAD_SUBKEY);
        unsafe {
            let mut hkey = HKEY::default();
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey_w.as_ptr()),
                0,
                KEY_READ | KEY_WRITE,
                &mut hkey,
            ).is_ok() {
                // Delete any zero-overrides that disabled slide gestures
                let stale_zero_keys = [
                    "ThreeFingerSlideUp",
                    "ThreeFingerSlideDown",
                    "ThreeFingerSlideLeft",
                    "ThreeFingerSlideRight",
                    "ThreeFingerTap",
                    "FourFingerSlideUp",
                    "FourFingerSlideDown",
                    "FourFingerSlideLeft",
                    "FourFingerSlideRight",
                    "FourFingerTap",
                    "ThreeFingerDownEnabled",
                    "FourFingerDownEnabled",
                ];

                for &val_name in &stale_zero_keys {
                    let val_name_w = to_wide_str(val_name);
                    let _ = RegDeleteValueW(hkey, PCWSTR(val_name_w.as_ptr()));
                }

                // Explicitly ensure standard gestures are enabled (1)
                let enabled_bytes = 1u32.to_le_bytes();
                for &val_name in &[
                    "ThreeFingerSlideEnabled",
                    "ThreeFingerTapEnabled",
                    "FourFingerSlideEnabled",
                    "FourFingerTapEnabled",
                ] {
                    let val_name_w = to_wide_str(val_name);
                    let _ = RegSetValueExW(
                        hkey,
                        PCWSTR(val_name_w.as_ptr()),
                        0,
                        REG_DWORD,
                        Some(&enabled_bytes),
                    );
                }

                let _ = RegCloseKey(hkey);
            }
        }
    }
}

impl Drop for TouchpadLock {
    fn drop(&mut self) {
        Self::restore_touchpad_gestures();
    }
}

// ============================================================================
// 3. Persistent Foreground Window Dominance
// ============================================================================

pub struct ForegroundLock {
    stop_signal: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
    target_window: Arc<Mutex<Option<isize>>>,
}

struct EnumKioskWndCtx {
    target_pids: HashSet<u32>,
    found_hwnd: Option<HWND>,
}

unsafe extern "system" fn enum_kiosk_wnd_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut EnumKioskWndCtx);
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));

    if ctx.target_pids.contains(&pid) {
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_ok() {
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;

            // Ensure this is a real render window, not a tiny tooltip or 0x0 offscreen frame
            // Ensure this is a real render window, not a tiny tooltip or 0x0 offscreen frame
            // Plan 23: Must be visible, must NOT be Chromium message/helper window (Chrome_WidgetWin_0, Cicero, etc.)
            let is_visible = unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(hwnd).as_bool() };
            if is_visible && width > 120 && height > 120 {
                let mut class_buf = [0u16; 256];
                let class_len = unsafe { windows::Win32::UI::WindowsAndMessaging::GetClassNameW(hwnd, &mut class_buf) };
                let class_name = String::from_utf16_lossy(&class_buf[..class_len as usize]);

                // Plan 23 F-2: Accept genuine Chrome (Chrome_WidgetWin_0) and Edge (Chrome_WidgetWin_1) viewports;
                // skip only hidden helper, worker, tooltip, and input helper frames.
                if !class_name.contains("Cicero")
                    && !class_name.contains("Tooltip")
                    && !class_name.contains("Worker")
                    && !class_name.contains("crashpad")
                    && !class_name.contains("UAC_Input")
                {
                    ctx.found_hwnd = Some(hwnd);
                    return BOOL(0); // Found genuine top-level kiosk UI window
                }
            }
        }
    }
    BOOL(1) // Keep enumerating
}

pub fn find_kiosk_window(target_pids: &HashSet<u32>) -> Option<HWND> {
    if target_pids.is_empty() {
        return None;
    }
    let mut ctx = EnumKioskWndCtx {
        target_pids: target_pids.clone(),
        found_hwnd: None,
    };
    unsafe {
        let _ = EnumWindows(
            Some(enum_kiosk_wnd_proc),
            LPARAM(&mut ctx as *mut _ as isize),
        );
    }
    ctx.found_hwnd
}

impl ForegroundLock {
    pub fn start(target_pids: Arc<Mutex<HashSet<u32>>>) -> Self {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_signal.clone();
        let target_window = Arc::new(Mutex::new(None));
        let window_clone = target_window.clone();

        let thread_handle = thread::spawn(move || {
            while !stop_clone.load(Ordering::Relaxed) {
                let pids = match target_pids.lock() {
                    Ok(p) => p.clone(),
                    Err(e) => e.into_inner().clone(),
                };

                if let Some(wnd) = find_kiosk_window(&pids) {
                    unsafe {
                        if let Ok(mut tw) = window_clone.lock() {
                            *tw = Some(wnd.0 as isize);
                        }
                        let fg = GetForegroundWindow();
                        if fg != wnd {
                            let _ = SetForegroundWindow(wnd);
                            let _ = BringWindowToTop(wnd);
                        }
                        let cx = GetSystemMetrics(SM_CXSCREEN);
                        let cy = GetSystemMetrics(SM_CYSCREEN);
                        let _ = SetWindowPos(
                            wnd,
                            HWND_TOPMOST,
                            0, 0, cx, cy,
                            SWP_SHOWWINDOW,
                        );
                    }
                }
                thread::sleep(Duration::from_millis(250));
            }
        });

        ForegroundLock {
            stop_signal,
            thread_handle: Some(thread_handle),
            target_window,
        }
    }
}

impl Drop for ForegroundLock {
    fn drop(&mut self) {
        self.stop_signal.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
        // Explicitly unpin the kiosk window from HWND_TOPMOST so host desktop is 100% restored
        unsafe {
            if let Ok(tw) = self.target_window.lock() {
                if let Some(val) = *tw {
                    let wnd = HWND(val as *mut std::ffi::c_void);
                    if !wnd.is_invalid() {
                        let _ = SetWindowPos(
                            wnd,
                            HWND_NOTOPMOST,
                            0, 0, 0, 0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                        );
                    }
                }
            }
        }
    }
}

// ============================================================================
// 4. System Clipboard Guard (Wiper)
// ============================================================================

pub struct ClipboardGuard {
    stop_signal: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

impl ClipboardGuard {
    pub fn start() -> Self {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_signal.clone();

        let thread_handle = thread::spawn(move || {
            while !stop_clone.load(Ordering::Relaxed) {
                unsafe {
                    if OpenClipboard(HWND(std::ptr::null_mut())).is_ok() {
                        let _ = EmptyClipboard();
                        let _ = CloseClipboard();
                    }
                }
                thread::sleep(Duration::from_millis(400));
            }
        });

        ClipboardGuard {
            stop_signal,
            thread_handle: Some(thread_handle),
        }
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        self.stop_signal.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

// ============================================================================
// 5. Active Process Watchdog (Cheat Process Killer)
// ============================================================================

pub use crate::pre_flight::PROHIBITED_PROCESSES as BLACKLISTED_PROCESSES;
const _OLD_BLACKLIST: &[&str] = &[
    "ollama.exe",
    "lmstudio.exe",
    "text-generation-webui",
    "discord.exe",
    "slack.exe",
    "telegram.exe",
    "whatsapp.exe",
    "cheatengine.exe",
    "cheatengine-x86_64.exe",
];

pub struct ProcessWatchdog {
    stop_signal: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
    _violations: Arc<Mutex<Vec<String>>>,
}

impl ProcessWatchdog {
    pub fn start(
        violations: Arc<Mutex<Vec<String>>>,
        kiosk_pids: Arc<std::sync::Mutex<std::collections::HashSet<u32>>>,
        browser_exe: Option<String>,
        is_production: bool,
    ) -> Self {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_signal.clone();
        let viol_clone = violations.clone();
        let pids_clone = kiosk_pids.clone();

        let thread_handle = thread::spawn(move || {
            while !stop_clone.load(Ordering::Relaxed) {
                Self::scan_and_terminate(&viol_clone, &pids_clone, browser_exe.as_deref(), is_production);
                thread::sleep(Duration::from_millis(250));
            }
        });

        ProcessWatchdog {
            stop_signal,
            thread_handle: Some(thread_handle),
            _violations: violations,
        }
    }

    fn scan_and_terminate(
        violations: &Arc<Mutex<Vec<String>>>,
        kiosk_pids: &Arc<std::sync::Mutex<std::collections::HashSet<u32>>>,
        browser_exe: Option<&str>,
        is_production: bool,
    ) {
        let own_pid = unsafe { windows::Win32::System::Threading::GetCurrentProcessId() };
        let protected_pids: std::collections::HashSet<u32> = {
            let p = match kiosk_pids.lock() {
                Ok(guard) => guard.clone(),
                Err(e) => e.into_inner().clone(),
            };
            let seeds: Vec<u32> = p.into_iter().collect();
            find_all_descendants(&seeds).into_iter().collect()
        };

        // Plan 23 F-1: Allowlist includes launched browser image name for runtime protection
        let mut allowlist = crate::policy::Allowlist::citadel_default();
        if let Some(b) = browser_exe {
            allowlist = allowlist.with_kiosk_browser(b);
        }
        let mut active_unauthorized = Vec::new();

        unsafe {
            let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
                Ok(h) => h,
                Err(_) => return,
            };

            let mut entry = PROCESSENTRY32W::default();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let pid = entry.th32ProcessID;

                    let exe_name = String::from_utf16_lossy(&entry.szExeFile)
                        .trim_matches(char::from(0))
                        .to_string();

                    if !allowlist.is_allowed(&exe_name, pid, own_pid, &protected_pids) {
                        eprintln!("[SECURITY VIOLATION] Unauthorized cheat tool / process detected: {} (PID: {})", exe_name, pid);

                        // Attempt automatic termination
                        let mut terminated = false;
                        if let Ok(hproc) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                            if TerminateProcess(hproc, 1).is_ok() {
                                terminated = true;
                            }
                            let _ = CloseHandle(hproc);
                        }

                        if terminated {
                            eprintln!("[SECURITY WATCHDOG] Successfully terminated unauthorized process: {} (PID: {})", exe_name, pid);
                            if let Ok(mut v_lock) = violations.lock() {
                                v_lock.push(format!("VIOLATION terminated_process name=\"{}\" pid={}", exe_name, pid));
                            }
                        } else {
                            eprintln!("[CRITICAL SECURITY WATCHDOG] FAILED to terminate unauthorized process: {} (PID: {})", exe_name, pid);
                            if let Ok(mut v_lock) = violations.lock() {
                                v_lock.push(format!("CRITICAL_VIOLATION active_unauthorized name=\"{}\" pid={}", exe_name, pid));
                            }
                            if is_production {
                                active_unauthorized.push(format!("{} (PID: {})", exe_name, pid));
                            }
                        }
                    }

                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snapshot);
        }

        // Production runtime gate (Finding C):
        // If an unauthorized process cannot be terminated, block exam progress
        // until the candidate / environment is verified clean.
        if is_production {
            if !active_unauthorized.is_empty() {
                crate::local_control::WORKSTATION_BLOCKED.store(true, Ordering::SeqCst);
                if let Ok(mut reason) = crate::local_control::WORKSTATION_BLOCKED_REASON.lock() {
                    *reason = active_unauthorized.join(", ");
                }
            } else {
                crate::local_control::WORKSTATION_BLOCKED.store(false, Ordering::SeqCst);
                if let Ok(mut reason) = crate::local_control::WORKSTATION_BLOCKED_REASON.lock() {
                    reason.clear();
                }
            }
        }
    }
}

impl Drop for ProcessWatchdog {
    fn drop(&mut self) {
        self.stop_signal.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

// ============================================================================
// 6. Kiosk Browser Launcher & Process Manager
// ============================================================================

pub fn is_pid_active(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    unsafe {
        if let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            let mut exit_code = 0u32;
            let ok = GetExitCodeProcess(h, &mut exit_code).is_ok();
            let _ = CloseHandle(h);
            ok && exit_code == 259
        } else {
            false
        }
    }
}

pub fn find_all_descendants(root_pids: &[u32]) -> Vec<u32> {
    let mut all_pids: std::collections::HashSet<u32> = root_pids.iter().copied().filter(|&p| p != 0).collect();
    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return all_pids.into_iter().collect(),
        };

        let mut entry = PROCESSENTRY32W::default();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        // Multi-pass transitive closure to capture all child and grandchild processes
        for _ in 0..5 {
            let mut added_any = false;
            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let pid = entry.th32ProcessID;
                    let ppid = entry.th32ParentProcessID;
                    if all_pids.contains(&ppid) && !all_pids.contains(&pid) {
                        all_pids.insert(pid);
                        added_any = true;
                    }
                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            if !added_any {
                break;
            }
        }
        let _ = CloseHandle(snapshot);
    }
    all_pids.into_iter().collect()
}

pub struct KioskProcess {
    pub h_process: HANDLE,
    pub h_launcher: HANDLE,
    pub h_thread: HANDLE,
    pub pid: u32,
    pub launcher_pid: u32,
    pub profile_dir: PathBuf,
    pub known_pids: Arc<std::sync::Mutex<std::collections::HashSet<u32>>>,
    pub browser_exe: String,
}

impl KioskProcess {
    pub fn is_alive(&self) -> bool {
        let mut pids = match self.known_pids.lock() {
            Ok(p) => p,
            Err(e) => e.into_inner(),
        };

        // Collect all multi-tier descendants spawned by known processes
        let seeds: Vec<u32> = pids.iter().copied().collect();
        let all_descendants = find_all_descendants(&seeds);
        for d in all_descendants {
            pids.insert(d);
        }

        // Retain only currently active processes
        pids.retain(|&pid| is_pid_active(pid));

        !pids.is_empty()
    }

    pub fn try_wait(&mut self) -> Result<Option<u32>, std::io::Error> {
        if self.is_alive() {
            Ok(None)
        } else {
            let mut exit_code = 0u32;
            unsafe {
                if !self.h_process.is_invalid() {
                    let _ = GetExitCodeProcess(self.h_process, &mut exit_code);
                }
            }
            Ok(Some(exit_code))
        }
    }

    pub fn terminate(&self) {
        let pids = match self.known_pids.lock() {
            Ok(p) => p,
            Err(e) => e.into_inner(),
        };
        let seeds: Vec<u32> = pids.iter().copied().collect();
        let all_pids = find_all_descendants(&seeds);

        for pid in all_pids {
            unsafe {
                if let Ok(hproc) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                    let _ = TerminateProcess(hproc, 1);
                    let _ = CloseHandle(hproc);
                }
            }
        }

        unsafe {
            if !self.h_process.is_invalid() {
                let _ = TerminateProcess(self.h_process, 1);
            }
            if !self.h_launcher.is_invalid() && self.h_launcher != self.h_process {
                let _ = TerminateProcess(self.h_launcher, 1);
            }
        }
        Self::kill_browser_tree(self.pid, self.launcher_pid);
    }

    pub fn kill(&mut self) -> Result<(), std::io::Error> {
        self.terminate();
        Ok(())
    }

    pub fn kill_browser_tree(main_pid: u32, launcher_pid: u32) {
        unsafe {
            let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
                Ok(h) => h,
                Err(_) => return,
            };

            let mut entry = PROCESSENTRY32W::default();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let pid = entry.th32ProcessID;
                    let ppid = entry.th32ParentProcessID;
                    let exe_name = String::from_utf16_lossy(&entry.szExeFile)
                        .trim_matches(char::from(0))
                        .to_lowercase();

                    if pid == main_pid
                        || pid == launcher_pid
                        || ppid == main_pid
                        || ppid == launcher_pid
                        || ((exe_name.contains("msedge") || exe_name.contains("chrome")) && (ppid == main_pid || ppid == launcher_pid))
                    {
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
}

impl Drop for KioskProcess {
    fn drop(&mut self) {
        unsafe {
            if !self.h_thread.is_invalid() {
                let _ = CloseHandle(self.h_thread);
            }
            if !self.h_launcher.is_invalid() {
                let _ = CloseHandle(self.h_launcher);
            }
            if !self.h_process.is_invalid() && self.h_process != self.h_launcher {
                let _ = CloseHandle(self.h_process);
            }
        }
    }
}

pub fn find_child_process(parent_pid: u32) -> Option<u32> {
    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return None,
        };

        let mut entry = PROCESSENTRY32W::default();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                if entry.th32ParentProcessID == parent_pid {
                    let child_pid = entry.th32ProcessID;
                    let _ = CloseHandle(snapshot);
                    return Some(child_pid);
                }
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
        None
    }
}

pub fn terminate_lingering_browser_processes() {
    unsafe {
        if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut entry = PROCESSENTRY32W::default();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

            if Process32FirstW(snapshot, &mut entry).is_ok() {
                loop {
                    let pid = entry.th32ProcessID;
                    let null_pos = entry
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.szExeFile.len());
                    let exe_name = String::from_utf16_lossy(&entry.szExeFile[..null_pos]).to_lowercase();

                    if exe_name == "msedge.exe" || exe_name == "chrome.exe" || exe_name == "brave.exe" {
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
    thread::sleep(Duration::from_millis(300));
}

pub fn find_browser_executable() -> Option<PathBuf> {
    let mut candidate_paths: Vec<PathBuf> = Vec::new();

    // 1. Program Files & Program Files (x86)
    if let Ok(pf) = std::env::var("ProgramFiles") {
        candidate_paths.push(PathBuf::from(&pf).join(r"Microsoft\Edge\Application\msedge.exe"));
        candidate_paths.push(PathBuf::from(&pf).join(r"Google\Chrome\Application\chrome.exe"));
        candidate_paths.push(PathBuf::from(&pf).join(r"BraveSoftware\Brave-Browser\Application\brave.exe"));
    }
    if let Ok(pfx86) = std::env::var("ProgramFiles(x86)") {
        candidate_paths.push(PathBuf::from(&pfx86).join(r"Microsoft\Edge\Application\msedge.exe"));
        candidate_paths.push(PathBuf::from(&pfx86).join(r"Google\Chrome\Application\chrome.exe"));
        candidate_paths.push(PathBuf::from(&pfx86).join(r"BraveSoftware\Brave-Browser\Application\brave.exe"));
    }

    // Hardcoded standard locations
    candidate_paths.push(PathBuf::from(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"));
    candidate_paths.push(PathBuf::from(r"C:\Program Files\Microsoft\Edge\Application\msedge.exe"));
    candidate_paths.push(PathBuf::from(r"C:\Program Files\Google\Chrome\Application\chrome.exe"));
    candidate_paths.push(PathBuf::from(r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe"));
    candidate_paths.push(PathBuf::from(r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe"));

    // 2. LocalAppData (Per-user browser installations)
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        candidate_paths.push(PathBuf::from(&local_app_data).join(r"Microsoft\Edge\Application\msedge.exe"));
        candidate_paths.push(PathBuf::from(&local_app_data).join(r"Google\Chrome\Application\chrome.exe"));
        candidate_paths.push(PathBuf::from(&local_app_data).join(r"BraveSoftware\Brave-Browser\Application\brave.exe"));
    }

    for path in candidate_paths {
        if path.exists() {
            return Some(path);
        }
    }

    None
}

/// Spawns an isolated full-screen kiosk browser directly on the designated desktop (e.g. Secure Desktop).
pub fn launch_kiosk_on_desktop(target_url: &str, desktop_name: Option<&str>) -> Result<KioskProcess, String> {
    // 1. Terminate lingering background browser processes (Startup Boost, background extensions)
    // This is critical on Windows 10/11: if a background msedge.exe is running via Startup Boost,
    // newly spawned msedge.exe will hand off the URL to the background headless instance and exit!
    terminate_lingering_browser_processes();

    let browser_path = find_browser_executable()
        .ok_or_else(|| "No supported browser (Edge/Chrome/Brave) found on system".to_string())?;

    let temp_profile = std::env::temp_dir().join(format!("citadel_kiosk_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_profile);

    eprintln!("[CITADEL CLIENT] Spawning exclusive kiosk browser via: {:?}", browser_path);
    eprintln!("[CITADEL CLIENT] Target desktop plane: {:?}", desktop_name.unwrap_or("Default"));
    eprintln!("[CITADEL CLIENT] Connecting to exam endpoint: {}", target_url);

    // KIOSK HARDENING & STABILITY ARGS:
    // 1. --no-sandbox: CRITICAL for elevated Administrator launch (prevents Chromium sandbox abort).
    // 2. --kiosk: Universal fullscreen kiosk across Chrome, Edge, and Brave.
    // 3. --disable-background-mode: Prevents background persistence / Startup Boost interference.
    // 4. --start-maximized + --window-position=0,0: Ensures immediate full screen coverage.
    // NOTE: GPU acceleration is deliberately left ENABLED (no --disable-gpu and no
    // software-rasterizer fallback). Edge v130+ exits immediately or renders a
    // blank/transparent kiosk window when GPU is disabled ÃƒÆ’Ã†â€™Ãƒâ€šÃ‚Â¢ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡Ãƒâ€šÃ‚Â¬ÃƒÆ’Ã‚Â¢ÃƒÂ¢Ã¢â‚¬Å¡Ã‚Â¬Ãƒâ€šÃ‚Â see
    // docs/architecture/CITADEL_SECURITY_ARCHITECTURE.md ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â§7/ÃƒÆ’Ã†â€™ÃƒÂ¢Ã¢â€šÂ¬Ã…Â¡ÃƒÆ’Ã¢â‚¬Å¡Ãƒâ€šÃ‚Â§8 ("Never disable
    // GPU flags"), verified empirically against Edge v153.
    let arg_parts = [
        format!("\"{}\"", browser_path.display()),
        format!("--user-data-dir=\"{}\"", temp_profile.display()),
        "--kiosk".to_string(),
        "--start-maximized".to_string(),
        "--window-position=0,0".to_string(),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--no-sandbox".to_string(),
        "--test-type".to_string(),
        "--disable-background-mode".to_string(),
        "--disable-extensions".to_string(),
        "--disable-component-update".to_string(),
        "--disable-sync".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-domain-reliability".to_string(),
        "--disable-speech-api".to_string(),
        "--no-service-autorun".to_string(),
        "--disable-pinch".to_string(),
        "--disable-context-menu".to_string(),
        "--overscroll-history-navigation=0".to_string(),
        "--disable-session-crashed-bubble".to_string(),
        "--disable-infobars".to_string(),
        "--disable-features=Translate,OptimizationHints,MediaRouter,EdgeCollections,EdgeShopping,Compose,msEdgeSidebarSupport,msSmartScreenProtection,msUnderside,msEdgeHub".to_string(),
        "--user-agent=\"CITADEL-Lockdown-Client/1.0 (Windows NT 10.0; Win64; x64; CitadelSecurityCore)\"".to_string(),
        format!("\"{}\"", target_url),
    ];
    let args = arg_parts.join(" ");

    let mut wide_cmd = to_wide_str(&args);
    let mut si = STARTUPINFOW::default();
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
    si.dwFlags = STARTF_USESHOWWINDOW;
    si.wShowWindow = SW_MAXIMIZE.0 as u16;

    let mut _wide_desktop = Vec::new();
    if let Some(dname) = desktop_name {
        let full_dname = if dname.contains('\\') {
            dname.to_string()
        } else {
            format!("WinSta0\\{}", dname)
        };
        _wide_desktop = to_wide_str(&full_dname);
        si.lpDesktop = PWSTR(_wide_desktop.as_mut_ptr());
    }

    let mut pi = PROCESS_INFORMATION::default();

    unsafe {
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
        ).map_err(|e| format!("Failed to spawn kiosk browser process: {:?}", e))?;
    }

    let launcher_pid = pi.dwProcessId;
    let h_launcher = pi.hProcess;
    let h_launcher_thread = pi.hThread;

    eprintln!("[CITADEL CLIENT] Browser launcher initiated (PID: {}). Awaiting browser window...", launcher_pid);

    let mut all_pids = std::collections::HashSet::new();
    if launcher_pid != 0 {
        all_pids.insert(launcher_pid);
    }

    let mut real_browser_pid = launcher_pid;
    let mut h_browser_process = h_launcher;
    let mut confirmed_alive = false;
    let mut window_activated = false;

    // Poll for up to 8 seconds to locate the live browser window and child processes
    for attempt in 0..80 {
        thread::sleep(Duration::from_millis(100));

        // 1. Gather all descendants of the launcher process
        let seeds: Vec<u32> = all_pids.iter().copied().collect();
        let descendants = find_all_descendants(&seeds);
        for d in descendants {
            all_pids.insert(d);
        }

        // 2. Locate child process
        if let Some(child_pid) = find_child_process(launcher_pid) {
            all_pids.insert(child_pid);
            if real_browser_pid == launcher_pid {
                real_browser_pid = child_pid;
                if let Ok(hproc) = unsafe {
                    OpenProcess(
                        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
                        false,
                        child_pid,
                    )
                } {
                    h_browser_process = hproc;
                }
            }
            confirmed_alive = true;
        }

        // 3. Find and forcefully activate the kiosk window
        if let Some(hwnd) = find_kiosk_window(&all_pids) {
            unsafe {
                let cx = GetSystemMetrics(SM_CXSCREEN);
                let cy = GetSystemMetrics(SM_CYSCREEN);
                let _ = ShowWindow(hwnd, SW_MAXIMIZE);
                let _ = SetForegroundWindow(hwnd);
                let _ = BringWindowToTop(hwnd);
                let _ = SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    0, 0, cx, cy,
                    SWP_SHOWWINDOW,
                );
            }
            confirmed_alive = true;
            window_activated = true;
            eprintln!("[CITADEL CLIENT] Browser kiosk window verified & activated via Win32 (attempt {})", attempt);
            break;
        }

        if is_pid_active(launcher_pid) {
            confirmed_alive = true;
        }
    }

    if !confirmed_alive && !is_pid_active(launcher_pid) {
        let mut exit_code = 0u32;
        let query_ok = unsafe { GetExitCodeProcess(h_launcher, &mut exit_code).is_ok() };
        if query_ok && exit_code != 259 && exit_code != 0 {
            return Err(format!(
                "Browser process terminated with error code {}. Please verify Edge/Chrome installation.",
                exit_code
            ));
        }
        return Err("Exam browser process failed to initialize within timeout.".into());
    }

    eprintln!("[CITADEL CLIENT] Kiosk process ready with {} tracked PID(s). Window activated: {}", all_pids.len(), window_activated);

    let browser_name = browser_path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("msedge.exe")
        .to_string();

    Ok(KioskProcess {
        h_process: h_browser_process,
        h_launcher,
        h_thread: h_launcher_thread,
        pid: real_browser_pid,
        launcher_pid,
        profile_dir: temp_profile,
        known_pids: Arc::new(std::sync::Mutex::new(all_pids)),
        browser_exe: browser_name,
    })
}

pub fn launch_kiosk(target_url: &str) -> Result<KioskProcess, String> {
    launch_kiosk_on_desktop(target_url, None)
}
