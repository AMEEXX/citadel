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
    PROCESS_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, STARTF_USESHOWWINDOW, STARTUPINFOW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, FindWindowW, GetForegroundWindow, GetSystemMetrics, GetWindowThreadProcessId,
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
        let _dev_mode = std::env::var("CITADEL_DEV_MODE").is_ok() || std::env::var("CARGO").is_ok();
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

                    // Whitelist: never terminate recovery utilities or citadel tools
                    if exe_name.contains("recovery") || exe_name.contains("citadel") {
                        continue;
                    }

                    for &banned in BLACKLISTED_PROCESSES {
                        if exe_name.contains(banned) {
                            let pid = entry.th32ProcessID;
                            if pid == own_pid {
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
    pub h_launcher: HANDLE,
    pub h_thread: HANDLE,
    pub pid: u32,
    pub launcher_pid: u32,
    pub profile_dir: PathBuf,
}

impl KioskProcess {
    pub fn is_alive(&self) -> bool {
        // 1. Check if the process handle is still reporting active
        if !self.h_process.is_invalid() {
            unsafe {
                let mut exit_code = 0u32;
                if GetExitCodeProcess(self.h_process, &mut exit_code).is_ok() && exit_code == 259 {
                    return true;
                }
            }
        }

        // 2. Check if the browser window is still present on screen
        unsafe {
            if let Ok(wnd) = FindWindowW(w!("Chrome_WidgetWin_1"), None) {
                if !wnd.is_invalid() {
                    return true;
                }
            }
        }

        // 3. Check if any process with launcher as parent is alive
        if let Some(child_pid) = find_child_process(self.launcher_pid) {
            if child_pid != 0 {
                return true;
            }
        }

        false
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

    // KIOSK HARDENING ARGS:
    // CRITICAL: GPU flags (--disable-gpu, --disable-gpu-compositing, etc.) are REMOVED.
    // Modern Edge/Chrome v130+ abort with code 0 if all GPU & software raster pipelines are stripped.
    let args = format!(
        "\"{}\" --user-data-dir=\"{}\" --new-window --kiosk --edge-kiosk-type=fullscreen          --no-first-run --no-default-browser-check --disable-pinch --disable-context-menu          --overscroll-history-navigation=0 --disable-extensions --disable-component-update          --disable-sync --disable-background-networking --disable-domain-reliability          --disable-speech-api --no-service-autorun --disable-background-mode          --disable-backgrounding-occluded-windows          --disable-features=Translate,OptimizationHints,MediaRouter,EdgeCollections,EdgeShopping,Compose,msEdgeSidebarSupport,msSmartScreenProtection,msUnderside,msEdgeHub          --user-agent=\"CITADEL-Lockdown-Client/1.0 (Windows NT 10.0; Win64; x64; CitadelSecurityCore)\"          \"{}\"",
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

    let launcher_pid = pi.dwProcessId;
    let h_launcher = pi.hProcess;
    let h_launcher_thread = pi.hThread;

    eprintln!("[CITADEL CLIENT] Browser launcher initiated (PID: {}). Awaiting browser window...", launcher_pid);

    // Modern Edge/Chrome uses multi-process delegation: the launcher process
    // spawns child processes and may exit with code 0 while the children run the browser.
    // We poll for up to 5 seconds to locate the live browser window and child process.
    let mut real_browser_pid = launcher_pid;
    let mut h_browser_process = h_launcher;
    let mut confirmed_alive = false;

    for _ in 0..50 {
        thread::sleep(Duration::from_millis(100));

        // 1. Check for Chromium top-level window
        unsafe {
            if let Ok(wnd) = FindWindowW(w!("Chrome_WidgetWin_1"), None) {
                if !wnd.is_invalid() {
                    let mut win_pid = 0u32;
                    GetWindowThreadProcessId(wnd, Some(&mut win_pid));
                    if win_pid != 0 {
                        real_browser_pid = win_pid;
                        if let Ok(hproc) = OpenProcess(
                            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
                            false,
                            win_pid,
                        ) {
                            h_browser_process = hproc;
                        }
                    }
                    confirmed_alive = true;
                    eprintln!("[CITADEL CLIENT] Browser window verified (HWND: {:?}, PID: {})", wnd.0, real_browser_pid);
                    break;
                }
            }
        }

        // 2. Check for child process of the launcher
        if let Some(child_pid) = find_child_process(launcher_pid) {
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
            confirmed_alive = true;
            eprintln!("[CITADEL CLIENT] Browser child process verified (PID: {})", child_pid);
            break;
        }
    }

    if !confirmed_alive {
        // Check if launcher encountered a true crash (non-zero, non-259 exit code)
        let mut exit_code = 0u32;
        let query_ok = unsafe { GetExitCodeProcess(h_launcher, &mut exit_code).is_ok() };
        if query_ok && exit_code != 259 && exit_code != 0 {
            return Err(format!(
                "Browser process terminated with error code {}. Please verify Edge/Chrome installation.",
                exit_code
            ));
        }

        // Final check: did window appear right at deadline?
        let window_check = unsafe { FindWindowW(w!("Chrome_WidgetWin_1"), None) };
        if window_check.is_err() || window_check.unwrap().is_invalid() {
            return Err("Exam browser window failed to initialize within timeout.".into());
        }
    }

    Ok(KioskProcess {
        h_process: h_browser_process,
        h_launcher,
        h_thread: h_launcher_thread,
        pid: real_browser_pid,
        launcher_pid,
        profile_dir: temp_profile,
    })
}

pub fn launch_kiosk(target_url: &str) -> Result<KioskProcess, String> {
    launch_kiosk_on_desktop(target_url, None)
}
