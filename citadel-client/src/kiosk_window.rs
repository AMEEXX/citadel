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

use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{BOOL, CloseHandle, HANDLE, HWND};
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_READ, KEY_WRITE, REG_DWORD, REG_VALUE_TYPE,
};
use windows::Win32::System::Threading::{
    CreateProcessW, GetExitCodeProcess, OpenProcess, TerminateProcess, CREATE_NEW_PROCESS_GROUP,
    PROCESS_INFORMATION, PROCESS_TERMINATE, STARTF_USESHOWWINDOW, STARTUPINFOW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, FindWindowW, GetForegroundWindow, GetSystemMetrics,
    SetForegroundWindow, SetWindowPos, ShowWindow, HWND_BOTTOM, HWND_TOPMOST, SM_CXSCREEN,
    SM_CYSCREEN, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, SW_MAXIMIZE,
    SW_SHOW,
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
    "ThreeFingerSlideEnabled",
    "ThreeFingerTapEnabled",
    "FourFingerSlideEnabled",
    "FourFingerTapEnabled",
    "ThreeFingerDownEnabled",
    "FourFingerDownEnabled",
];

pub struct TouchpadLock {
    saved_values: HashMap<String, u32>,
}

fn to_wide_str(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

impl TouchpadLock {
    pub fn acquire() -> Self {
        let mut saved = HashMap::new();
        let subkey_w = to_wide_str(TOUCHPAD_REG_SUBKEY);

        unsafe {
            let mut hkey = HKEY::default();
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey_w.as_ptr()),
                0,
                KEY_READ | KEY_WRITE,
                &mut hkey,
            ).is_ok() {
                for &val_name in TOUCHPAD_GESTURE_KEYS {
                    let val_name_w = to_wide_str(val_name);
                    let mut val_type = REG_VALUE_TYPE::default();
                    let mut data_buf = [0u8; 4];
                    let mut data_len = 4u32;

                    if RegQueryValueExW(
                        hkey,
                        PCWSTR(val_name_w.as_ptr()),
                        None,
                        Some(&mut val_type),
                        Some(data_buf.as_mut_ptr()),
                        Some(&mut data_len),
                    ).is_ok() && val_type == REG_DWORD && data_len == 4 {
                        let original_val = u32::from_le_bytes(data_buf);
                        saved.insert(val_name.to_string(), original_val);
                    }

                    // Zero out the multi-finger gesture capability
                    let zero_bytes = 0u32.to_le_bytes();
                    let _ = RegSetValueExW(
                        hkey,
                        PCWSTR(val_name_w.as_ptr()),
                        0,
                        REG_DWORD,
                        Some(&zero_bytes),
                    );
                }
                let _ = RegCloseKey(hkey);
            }
        }

        TouchpadLock { saved_values: saved }
    }
}

impl Drop for TouchpadLock {
    fn drop(&mut self) {
        let subkey_w = to_wide_str(TOUCHPAD_REG_SUBKEY);
        unsafe {
            let mut hkey = HKEY::default();
            if RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey_w.as_ptr()),
                0,
                KEY_WRITE,
                &mut hkey,
            ).is_ok() {
                for (name, val) in &self.saved_values {
                    let name_w = to_wide_str(name);
                    let bytes = val.to_le_bytes();
                    let _ = RegSetValueExW(
                        hkey,
                        PCWSTR(name_w.as_ptr()),
                        0,
                        REG_DWORD,
                        Some(&bytes),
                    );
                }
                let _ = RegCloseKey(hkey);
            }
        }
    }
}

// ============================================================================
// 3. Persistent Foreground Window Dominance
// ============================================================================

pub struct ForegroundLock {
    stop_signal: Arc<AtomicBool>,
    thread_handle: Option<JoinHandle<()>>,
}

impl ForegroundLock {
    pub fn start() -> Self {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let stop_clone = stop_signal.clone();

        let thread_handle = thread::spawn(move || {
            while !stop_clone.load(Ordering::Relaxed) {
                unsafe {
                    let hwnd = FindWindowW(w!("Chrome_WidgetWin_1"), None);
                    if let Ok(wnd) = hwnd {
                        if !wnd.is_invalid() {
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
                }
                thread::sleep(Duration::from_millis(150));
            }
        });

        ForegroundLock {
            stop_signal,
            thread_handle: Some(thread_handle),
        }
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

                            if let Ok(hproc) = OpenProcess(PROCESS_TERMINATE, false, pid) {
                                let _ = TerminateProcess(hproc, 1);
                                let _ = CloseHandle(hproc);
                            }
                            break;
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

pub struct KioskProcess {
    pub h_process: HANDLE,
    pub h_thread: HANDLE,
    pub pid: u32,
}

impl KioskProcess {
    pub fn is_alive(&self) -> bool {
        unsafe {
            let mut exit_code = 0u32;
            if GetExitCodeProcess(self.h_process, &mut exit_code).is_ok() {
                exit_code == 259 // STILL_ACTIVE
            } else {
                false
            }
        }
    }

    pub fn try_wait(&mut self) -> Result<Option<u32>, std::io::Error> {
        unsafe {
            let mut exit_code = 0u32;
            if GetExitCodeProcess(self.h_process, &mut exit_code).is_ok() {
                if exit_code == 259 {
                    Ok(None)
                } else {
                    Ok(Some(exit_code))
                }
            } else {
                Err(std::io::Error::last_os_error())
            }
        }
    }

    pub fn terminate(&self) {
        unsafe {
            let _ = TerminateProcess(self.h_process, 1);
        }
    }

    pub fn kill(&mut self) -> Result<(), std::io::Error> {
        self.terminate();
        Ok(())
    }
}

impl Drop for KioskProcess {
    fn drop(&mut self) {
        unsafe {
            if !self.h_thread.is_invalid() {
                let _ = CloseHandle(self.h_thread);
            }
            if !self.h_process.is_invalid() {
                let _ = CloseHandle(self.h_process);
            }
        }
    }
}

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

/// Spawns an isolated full-screen kiosk browser directly on the designated desktop (e.g. Secure Desktop).
pub fn launch_kiosk_on_desktop(target_url: &str, desktop_name: Option<&str>) -> Result<KioskProcess, String> {
    let browser_path = find_browser_executable()
        .ok_or_else(|| "No supported browser (Edge/Chrome) found on system".to_string())?;

    let temp_profile = std::env::temp_dir().join(format!("citadel_kiosk_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_profile);

    eprintln!("[CITADEL CLIENT] Spawning exclusive kiosk browser via: {:?}", browser_path);
    eprintln!("[CITADEL CLIENT] Target desktop plane: {:?}", desktop_name.unwrap_or("Default"));
    eprintln!("[CITADEL CLIENT] Connecting to exam endpoint: {}", target_url);

    let args = format!(
        "\"{}\" --user-data-dir=\"{}\" --new-window --kiosk --edge-kiosk-type=fullscreen \
         --no-first-run --no-default-browser-check --disable-pinch --disable-context-menu \
         --overscroll-history-navigation=0 --disable-extensions --disable-component-update \
         --disable-sync --disable-background-networking --disable-domain-reliability \
         --disable-speech-api --disable-gpu --disable-gpu-compositing --disable-software-rasterizer \
         --disable-d3d11 --disable-accelerated-2d-canvas \
         --no-service-autorun --disable-background-mode --disable-backgrounding-occluded-windows \
         --disable-features=Translate,OptimizationHints,MediaRouter,EdgeCollections,EdgeShopping,Compose,msEdgeSidebarSupport,msSmartScreenProtection,msUnderside,msEdgeHub \
         --user-agent=\"CITADEL-Lockdown-Client/1.0 (Windows NT 10.0; Win64; x64; CitadelSecurityCore)\" \
         \"{}\"",
        browser_path.display(),
        temp_profile.display(),
        target_url
    );

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

    let kiosk = KioskProcess {
        h_process: pi.hProcess,
        h_thread: pi.hThread,
        pid: pi.dwProcessId,
    };

    // Verify browser did not terminate immediately on launch (e.g. GPU crash or delegation exit)
    std::thread::sleep(Duration::from_millis(1500));
    unsafe {
        let mut exit_code = 0u32;
        if GetExitCodeProcess(kiosk.h_process, &mut exit_code).is_ok() && exit_code != 259 {
            return Err(format!(
                "Browser process exited immediately after launch with code {}.                  Kiosk cannot render on this display configuration.",
                exit_code
            ));
        }
    }

    Ok(kiosk)
}

pub fn launch_kiosk(target_url: &str) -> Result<KioskProcess, String> {
    launch_kiosk_on_desktop(target_url, None)
}
