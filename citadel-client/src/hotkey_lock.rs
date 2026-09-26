//! CITADEL Client Module: Hardened System Hotkey Suppression with Health Monitor
//!
//! Intercepts and drops escape hotkeys (Alt+Tab, Win Key, Win combinations,
//! Ctrl+Esc, Alt+F4, PrintScreen, DevTools, and browser escape shortcuts)
//! on the active desktop.
//!
//! Features:
//! 1. Fast zero-allocation hook callback path (no mutexes, no stdout/stderr I/O).
//! 2. Associates with the isolated Secure Desktop via SetThreadDesktop.
//! 3. Active 5-second synthetic pulse health watchdog: detects silent hook removal
//!    by the Windows OS and automatically reinstalls the hook without user disruption.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::StationsAndDesktops::{SetThreadDesktop, HDESK};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_KEYUP, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_QUIT,
};

const HEALTH_CHECK_VK: u32 = 0x87; // VK_F24 (harmless synthetic ping key)

static EMERGENCY_OVERRIDE_TRIGGERED: AtomicBool = AtomicBool::new(false);
static HEALTH_PONG_RECEIVED: AtomicBool = AtomicBool::new(false);
static HOOK_REINSTALL_REQUESTED: AtomicBool = AtomicBool::new(false);
static ESC_TAP_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static LAST_ESC_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn check_escape_rapid_press() -> bool {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let last_ms = LAST_ESC_MS.swap(now_ms, Ordering::SeqCst);
    if now_ms.saturating_sub(last_ms) < 700 {
        let count = ESC_TAP_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
        if count >= 5 {
            ESC_TAP_COUNT.store(0, Ordering::SeqCst);
            return true;
        }
    } else {
        ESC_TAP_COUNT.store(1, Ordering::SeqCst);
    }
    false
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Allow,
    Suppress,
    EmergencyOverride,
    HealthPong,
}

#[inline(always)]
pub fn evaluate_keystroke(
    vk: u32,
    flags: u32,
    ctrl_pressed: bool,
    shift_pressed: bool,
    win_pressed: bool,
) -> KeyAction {
    // Health monitor ping
    if vk == HEALTH_CHECK_VK {
        return KeyAction::HealthPong;
    }

    // Rapid 5x Escape Emergency Override (tap Escape 5 times rapidly)
    if vk == 0x1B && check_escape_rapid_press() {
        return KeyAction::EmergencyOverride;
    }

    let alt_down = (flags & 0x20) != 0;

    // Proctor Emergency Override A: Ctrl + Shift + Alt + Q (VK 0x51 - easy, no Fn key)
    if vk == 0x51 && ctrl_pressed && shift_pressed && alt_down {
        return KeyAction::EmergencyOverride;
    }

    // Proctor Emergency Override B: Ctrl + Shift + Alt + F12 (VK 0x7B)
    if vk == 0x7B && ctrl_pressed && shift_pressed && alt_down {
        return KeyAction::EmergencyOverride;
    }

    // 1. Suppress ANY key combination when the Windows key is down
    if win_pressed {
        return KeyAction::Suppress;
    }

    // 2. Suppress Windows Keys themselves (LWIN: 0x5B, RWIN: 0x5C)
    if vk == 0x5B || vk == 0x5C {
        return KeyAction::Suppress;
    }

    // 3. Suppress Application / Context Menu key (VK_APPS: 0x5D)
    if vk == 0x5D {
        return KeyAction::Suppress;
    }

    // 4. Suppress PrintScreen / Screen Capture (VK_SNAPSHOT: 0x2C)
    if vk == 0x2C {
        return KeyAction::Suppress;
    }

    // 5. Suppress Alt-based system switching combinations
    if alt_down {
        match vk {
            0x09 // Alt + Tab
            | 0x1B // Alt + Escape
            | 0x73 // Alt + F4
            | 0x20 // Alt + Space
            | 0x0D => return KeyAction::Suppress, // Alt + Enter
            _ => {}
        }
    }

    // 6. Suppress Ctrl-based system and browser escape combinations
    if ctrl_pressed {
        match vk {
            0x1B // Ctrl + Escape or Ctrl + Shift + Escape
            | 0x4E // N (New Window)
            | 0x54 // T (New Tab)
            | 0x57 // W (Close Tab)
            | 0x4A // J (Downloads)
            | 0x48 // H (History)
            | 0x4F // O (Open File)
            | 0x4C // L (Address Bar)
            | 0x55 // U (View Source)
            | 0x50 // P (Print)
            | 0x53 => return KeyAction::Suppress, // S (Save)
            _ => {}
        }
    }

    // 7. Suppress Function keys used for browser controls
    match vk {
        0x70 // F1: Help
        | 0x72 // F3: Find in page
        | 0x74 // F5: Reload
        | 0x7A // F11: Fullscreen toggle
        | 0x7B => KeyAction::Suppress, // F12: DevTools
        _ => KeyAction::Allow,
    }
}

pub fn is_emergency_override_triggered() -> bool {
    EMERGENCY_OVERRIDE_TRIGGERED.load(Ordering::SeqCst)
}

pub fn reset_emergency_override() {
    EMERGENCY_OVERRIDE_TRIGGERED.store(false, Ordering::SeqCst);
}

unsafe extern "system" fn hotkey_hook_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let kbd = *(lparam.0 as *const KBDLLHOOKSTRUCT);

        // Fast state extraction
        let ctrl_pressed = (GetAsyncKeyState(0x11) as i16) < 0;
        let shift_pressed = (GetAsyncKeyState(0x10) as i16) < 0;
        let win_pressed = (GetAsyncKeyState(0x5B) as i16) < 0 || (GetAsyncKeyState(0x5C) as i16) < 0;

        match evaluate_keystroke(kbd.vkCode, kbd.flags.0, ctrl_pressed, shift_pressed, win_pressed) {
            KeyAction::Suppress => return LRESULT(1),
            KeyAction::HealthPong => {
                HEALTH_PONG_RECEIVED.store(true, Ordering::SeqCst);
                return LRESULT(1);
            }
            KeyAction::EmergencyOverride => {
                EMERGENCY_OVERRIDE_TRIGGERED.store(true, Ordering::SeqCst);
                return LRESULT(1);
            }
            KeyAction::Allow => {}
        }
    }

    CallNextHookEx(HHOOK(std::ptr::null_mut()), code, wparam, lparam)
}

fn send_synthetic_health_probe() {
    unsafe {
        let mut input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(HEALTH_CHECK_VK as u16),
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        let _ = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);

        input.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
        let _ = SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

struct SendHdesk(HDESK);
unsafe impl Send for SendHdesk {}

pub struct HotkeyLockHandle {
    thread_id: u32,
    join_handle: Option<JoinHandle<()>>,
    watchdog_handle: Option<JoinHandle<()>>,
    stop_signal: std::sync::Arc<AtomicBool>,
}

impl HotkeyLockHandle {
    pub fn stop(mut self) {
        self.cleanup();
    }

    fn cleanup(&mut self) {
        self.stop_signal.store(true, Ordering::Relaxed);
        if self.thread_id != 0 {
            unsafe {
                let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
            self.thread_id = 0;
        }
        if let Some(h) = self.watchdog_handle.take() {
            let _ = h.join();
        }
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for HotkeyLockHandle {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Installs the system-wide hotkey suppression hook with continuous health monitoring.
/// If `desktop_handle` is provided, binds the hook thread to that desktop first.
pub fn install_hotkey_lock_with_desktop(desktop: Option<HDESK>) -> Result<HotkeyLockHandle, String> {
    reset_emergency_override();
    let (tx, rx) = mpsc::channel::<Result<u32, String>>();
    let stop_signal = std::sync::Arc::new(AtomicBool::new(false));
    let stop_clone = stop_signal.clone();
    let send_desktop = desktop.map(SendHdesk);

    // Spawn dedicated STA message pump thread
    let join_handle = thread::spawn(move || {
        if let Some(d) = send_desktop {
            unsafe {
                let _ = SetThreadDesktop(d.0);
            }
        }

        let thread_id = unsafe { GetCurrentThreadId() };

        let mut hhook = unsafe {
            SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(hotkey_hook_proc),
                None,
                0,
            )
        };

        let current_hhook = match hhook {
            Ok(h) => {
                let _ = tx.send(Ok(thread_id));
                h
            }
            Err(e) => {
                let _ = tx.send(Err(format!("SetWindowsHookExW failed: {:?}", e)));
                return;
            }
        };

        hhook = Ok(current_hhook);

        let mut msg = MSG::default();
        while unsafe { GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0).as_bool() } {
            // Check if watchdog requested hook re-installation
            if HOOK_REINSTALL_REQUESTED.swap(false, Ordering::SeqCst) {
                if let Ok(old_h) = hhook {
                    unsafe {
                        let _ = UnhookWindowsHookEx(old_h);
                    }
                }
                hhook = unsafe {
                    SetWindowsHookExW(
                        WH_KEYBOARD_LL,
                        Some(hotkey_hook_proc),
                        None,
                        0,
                    )
                };
                eprintln!("[CITADEL CLIENT] KEYBOARD HOOK REINSTALLED BY HEALTH MONITOR.");
            }

            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        if let Ok(h) = hhook {
            unsafe {
                let _ = UnhookWindowsHookEx(h);
            }
        }
    });

    let thread_id = rx
        .recv()
        .map_err(|e| format!("Failed to initialize hotkey hook thread: {}", e))??;

    // Spawn health monitor watchdog thread (pings every 5 seconds)
    let watchdog_handle = thread::spawn(move || {
        while !stop_clone.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_secs(5));
            if stop_clone.load(Ordering::Relaxed) {
                break;
            }

            HEALTH_PONG_RECEIVED.store(false, Ordering::SeqCst);
            send_synthetic_health_probe();

            // Wait 500ms for hook to process synthetic probe
            thread::sleep(Duration::from_millis(500));

            if !HEALTH_PONG_RECEIVED.load(Ordering::SeqCst) && !stop_clone.load(Ordering::Relaxed) {
                eprintln!("[HOTKEY WATCHDOG] Warning: Keyboard hook heartbeat lost! Requesting reinstallation...");
                HOOK_REINSTALL_REQUESTED.store(true, Ordering::SeqCst);
                // Wake up thread's message loop
                unsafe {
                    let _ = PostThreadMessageW(thread_id, 0x0000, WPARAM(0), LPARAM(0)); // WM_NULL
                }
            }
        }
    });

    Ok(HotkeyLockHandle {
        thread_id,
        join_handle: Some(join_handle),
        watchdog_handle: Some(watchdog_handle),
        stop_signal,
    })
}

pub fn install_hotkey_lock() -> Result<HotkeyLockHandle, String> {
    install_hotkey_lock_with_desktop(None)
}
