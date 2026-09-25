//! CITADEL Client Kiosk Window Manager & Shell Hardening
//!
//! Enforces:
//! 1. Full-screen isolated kiosk browser window launch with Chromium security flags
//! 2. Persistent Taskbar suppression (SW_HIDE + EnableWindow(false) + HWND_BOTTOM)
//! 3. Precision Touchpad gesture suppression (disables 3-finger and 4-finger swipes via Registry)
//! 4. Continuous Foreground window dominance (locks kiosk window to HWND_TOPMOST)
//! 5. System clipboard wiper (prevents external copy/paste data leakage)
//! 6. Active process watchdog (detects and terminates blacklisted cheat processes)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{BOOL, HWND};
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_READ, KEY_WRITE, REG_DWORD, REG_VALUE_TYPE,
};
use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, FindWindowW, GetForegroundWindow, GetSystemMetrics,
    SetForegroundWindow, SetWindowPos, ShowWindow, HWND_BOTTOM, HWND_TOPMOST, SM_CXSCREEN,
    SM_CYSCREEN, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW,
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
// 2. Touchpad Gesture Suppression (3-finger & 4-finger swipes)
// ============================================================================

const TOUCHPAD_REG_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\PrecisionTouchPad";

const TOUCHPAD_GESTURE_KEYS: &[&str] = &[
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
];

/// RAII Guard that disables multi-finger gestures in Windows Precision Touchpad settings
/// and restores the candidate's original registry configuration upon exit.
pub struct TouchpadLock {
    backup_values: HashMap<String, u32>,
}

impl TouchpadLock {
    pub fn acquire() -> Self {
        let mut backup_values = HashMap::new();

        let subkey_wide: Vec<u16> = TOUCHPAD_REG_SUBKEY.encode_utf16().chain(std::iter::once(0)).collect();
        let mut hkey = HKEY::default();

        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey_wide.as_ptr()),
                0,
                KEY_READ | KEY_WRITE,
                &mut hkey,
            )
        };

        if status.is_ok() && !hkey.is_invalid() {
            for key_name in TOUCHPAD_GESTURE_KEYS {
                let name_wide: Vec<u16> = key_name.encode_utf16().chain(std::iter::once(0)).collect();
                let mut val_type = REG_VALUE_TYPE::default();
                let mut data: u32 = 0;
                let mut size: u32 = std::mem::size_of::<u32>() as u32;

                let query = unsafe {
                    RegQueryValueExW(
                        hkey,
                        PCWSTR(name_wide.as_ptr()),
                        None,
                        Some(&mut val_type),
                        Some(&mut data as *mut _ as *mut u8),
                        Some(&mut size),
                    )
                };

                if query.is_ok() {
                    backup_values.insert((*key_name).to_string(), data);
                }

                // Write 0 to disable gesture
                let zero: u32 = 0;
                let _ = unsafe {
                    RegSetValueExW(
                        hkey,
                        PCWSTR(name_wide.as_ptr()),
                        0,
                        REG_DWORD,
                        Some(std::slice::from_raw_parts(&zero as *const _ as *const u8, std::mem::size_of::<u32>())),
                    )
                };
            }

            unsafe {
                let _ = RegCloseKey(hkey);
            }
        }

        TouchpadLock { backup_values }
    }
}

impl Drop for TouchpadLock {
    fn drop(&mut self) {
        if self.backup_values.is_empty() {
            return;
        }

        let subkey_wide: Vec<u16> = TOUCHPAD_REG_SUBKEY.encode_utf16().chain(std::iter::once(0)).collect();
        let mut hkey = HKEY::default();

        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey_wide.as_ptr()),
                0,
                KEY_WRITE,
                &mut hkey,
            )
        };

        if status.is_ok() && !hkey.is_invalid() {
            for (key_name, original_val) in &self.backup_values {
                let name_wide: Vec<u16> = key_name.encode_utf16().chain(std::iter::once(0)).collect();
                let _ = unsafe {
                    RegSetValueExW(
                        hkey,
                        PCWSTR(name_wide.as_ptr()),
                        0,
                        REG_DWORD,
                        Some(std::slice::from_raw_parts(original_val as *const _ as *const u8, std::mem::size_of::<u32>())),
                    )
                };
            }
            unsafe {
                let _ = RegCloseKey(hkey);
            }
        }
    }
}

// ============================================================================
// 3. Foreground Dominance Enforcer
// ============================================================================

/// RAII Guard that pins the assessment kiosk window as HWND_TOPMOST and forces it to
/// remain in the foreground, recovering from any focus-loss or attempt to switch windows.
pub struct ForegroundLock {
    stop_signal: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

impl ForegroundLock {
    pub fn start() -> Self {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_signal.clone();

        let thread_handle = thread::spawn(move || {
            let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
            let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };

            while !stop_clone.load(Ordering::Relaxed) {
                // Find Chromium / Edge kiosk window
                if let Some(kiosk_hwnd) = Self::find_kiosk_hwnd() {
                    unsafe {
                        // Enforce HWND_TOPMOST and fullscreen bounds
                        let _ = SetWindowPos(
                            kiosk_hwnd,
                            HWND_TOPMOST,
                            0, 0, screen_w, screen_h,
                            SWP_SHOWWINDOW,
                        );

                        // If not current foreground window, yank focus back immediately
                        let fg = GetForegroundWindow();
                        if fg != kiosk_hwnd {
                            let _ = SetForegroundWindow(kiosk_hwnd);
                            let _ = BringWindowToTop(kiosk_hwnd);
                        }
                    }
                }
                thread::sleep(Duration::from_millis(150));
            }
        });

        ForegroundLock {
            stop_signal,
            thread_handle: Some(thread_handle),
        }
    }

    fn find_kiosk_hwnd() -> Option<HWND> {
        unsafe {
            // Check Chrome_WidgetWin_1 (Chromium / Edge main window class)
            if let Ok(hwnd) = FindWindowW(w!("Chrome_WidgetWin_1"), None) {
                if !hwnd.is_invalid() {
                    return Some(hwnd);
                }
            }
        }
        None
    }
}

impl Drop for ForegroundLock {
    fn drop(&mut self) {
        self.stop_signal.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

// ============================================================================
// 4. System Clipboard Guard (Wiper)
// ============================================================================

/// RAII Guard that periodically flushes the system clipboard to prevent
/// candidates from copying question text to other applications or pasting external answers.
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

const BLACKLISTED_PROCESSES: &[&str] = &[
    "taskmgr.exe",
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
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
    pub fn start(violations: Arc<Mutex<Vec<String>>>) -> Self {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_signal.clone();
        let viol_clone = violations.clone();

        let thread_handle = thread::spawn(move || {
            while !stop_clone.load(Ordering::Relaxed) {
                Self::scan_and_terminate(&viol_clone);
                thread::sleep(Duration::from_millis(1000));
            }
        });

        ProcessWatchdog {
            stop_signal,
            thread_handle: Some(thread_handle),
            _violations: violations,
        }
    }

    fn scan_and_terminate(violations: &Arc<Mutex<Vec<String>>>) {
        let own_pid = unsafe { windows::Win32::System::Threading::GetCurrentProcessId() };
        let dev_mode = std::env::var("CITADEL_DEV_MODE").is_ok() || std::env::var("CARGO").is_ok();
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

                    for &banned in BLACKLISTED_PROCESSES {
                        if exe_name.contains(banned) {
                            let pid = entry.th32ProcessID;
                            if pid == own_pid || (dev_mode && (exe_name.contains("powershell") || exe_name.contains("cmd"))) {
                                continue;
                            }
                            eprintln!("[SECURITY VIOLATION] Unauthorized cheat tool detected: {} (PID: {})", exe_name, pid);

                            if let Ok(mut v_lock) = violations.lock() {
                                v_lock.push(format!("VIOLATION blacklisted_process name=\"{}\" pid={}", exe_name, pid));
                            }

                            // Terminate the unauthorized process
                            if let Ok(hproc) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                                let _ = TerminateProcess(hproc, 1);
                            }
                            break;
                        }
                    }

                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
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
// 6. Kiosk Browser Launcher
// ============================================================================

pub fn find_browser_executable() -> Option<PathBuf> {
    let candidate_paths = [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
    ];

    for path in candidate_paths {
        let p = Path::new(path);
        if p.exists() {
            return Some(p.to_path_buf());
        }
    }

    None
}

pub fn launch_kiosk(target_url: &str) -> std::io::Result<Child> {
    let browser_path = find_browser_executable()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "No supported browser (Edge/Chrome) found on system"))?;

    let temp_profile = std::env::temp_dir().join(format!("citadel_kiosk_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_profile);

    println!("[CITADEL CLIENT] Spawning exclusive kiosk window via: {:?}", browser_path);
    println!("[CITADEL CLIENT] Connecting to exam endpoint: {}", target_url);

    // CRITICAL CHROMIUM SECURITY FLAGS:
    // 1. --user-data-dir and --new-window ensure a standalone, isolated process.
    // 2. --kiosk and --edge-kiosk-type=fullscreen lock full screen without window controls.
    // 3. --overscroll-history-navigation=0 blocks 2-finger swipe navigation.
    // 4. --disable-extensions and --disable-pinch prevent unauthorized tooling.
    // 5. target_url is passed directly as an argument, NOT --app=.
    let child = Command::new(browser_path)
        .arg(format!("--user-data-dir={}", temp_profile.display()))
        .arg("--new-window")
        .arg("--kiosk")
        .arg("--edge-kiosk-type=fullscreen")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-pinch")
        .arg("--disable-context-menu")
        .arg("--overscroll-history-navigation=0")
        .arg("--disable-extensions")
        .arg("--disable-component-update")
        .arg("--disable-sync")
        .arg("--disable-background-networking")
        .arg("--disable-domain-reliability")
        .arg("--disable-speech-api")
        .arg("--disable-features=Translate,OptimizationHints,MediaRouter,EdgeCollections,EdgeShopping,Compose,msEdgeSidebarSupport,msSmartScreenProtection,msUnderside,msEdgeHub")
        .arg("--user-agent=CITADEL-Lockdown-Client/1.0 (Windows NT 10.0; Win64; x64; CitadelSecurityCore)")
        .arg(target_url)
        .spawn()?;

    Ok(child)
}
