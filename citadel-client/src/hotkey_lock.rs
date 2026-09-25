//! CITADEL Client Module: System Hotkey Suppression
//!
//! Intercepts and drops escape hotkeys (Alt+Tab, Win Key, Win combinations,
//! Ctrl+Esc, Alt+F4, PrintScreen, browser escape shortcuts, and Task View triggers)
//! to prevent the candidate from leaving or breaking out of the kiosk assessment window.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_QUIT,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Allow,
    Suppress,
    EmergencyOverride,
}

/// Pure evaluation function for system keystroke filtering.
/// Distinguishes between normal coding inputs (letters, digits, symbols, Tab, Enter, Backspace, Arrows)
/// and system breakout combinations.
pub fn evaluate_keystroke(
    vk: u32,
    flags: u32,
    ctrl_pressed: bool,
    shift_pressed: bool,
    win_pressed: bool,
) -> KeyAction {
    let alt_down = (flags & 0x20) != 0;

    // Proctor Emergency Override: Ctrl + Shift + Alt + F12 (VK 0x7B)
    if vk == 0x7B && ctrl_pressed && shift_pressed && alt_down {
        return KeyAction::EmergencyOverride;
    }

    // 1. Suppress ANY key combination when the Windows key is down (Win+Tab, Win+D, Win+R, Win+E, Win+X, Win+V, etc.)
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
        // Alt + Tab (0x09)
        if vk == 0x09 {
            return KeyAction::Suppress;
        }
        // Alt + Escape (0x1B)
        if vk == 0x1B {
            return KeyAction::Suppress;
        }
        // Alt + F4 (0x73)
        if vk == 0x73 {
            return KeyAction::Suppress;
        }
        // Alt + Space (0x20 - Window System Menu)
        if vk == 0x20 {
            return KeyAction::Suppress;
        }
        // Alt + Enter (0x0D - Window property toggle)
        if vk == 0x0D {
            return KeyAction::Suppress;
        }
    }

    // 6. Suppress Ctrl-based system and browser escape combinations
    if ctrl_pressed {
        // Ctrl + Escape (0x1B - Start Menu) or Ctrl + Shift + Escape (Task Manager)
        if vk == 0x1B {
            return KeyAction::Suppress;
        }
        // Browser navigation shortcuts:
        // N (0x4E - New Window), T (0x54 - New Tab), W (0x57 - Close Window/Tab),
        // J (0x4A - Downloads), H (0x48 - History), O (0x4F - Open File),
        // L (0x4C - Address Bar), U (0x55 - View Source), P (0x50 - Print), S (0x53 - Save)
        match vk {
            0x4E | 0x54 | 0x57 | 0x4A | 0x48 | 0x4F | 0x4C | 0x55 | 0x50 | 0x53 => {
                return KeyAction::Suppress;
            }
            _ => {}
        }
    }

    // 7. Suppress Function keys used for browser controls
    match vk {
        0x70 => KeyAction::Suppress, // F1: Help / Support window
        0x72 => KeyAction::Suppress, // F3: Find in page
        0x74 => KeyAction::Suppress, // F5: Reload exam page
        0x7A => KeyAction::Suppress, // F11: Fullscreen toggle
        0x7B => KeyAction::Suppress, // F12: DevTools
        _ => KeyAction::Allow,
    }
}

static EMERGENCY_OVERRIDE_TRIGGERED: AtomicBool = AtomicBool::new(false);

/// Checks if the proctor emergency override combination was triggered.
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
        let ctrl_pressed = (GetAsyncKeyState(0x11) as i16) < 0; // VK_CONTROL
        let shift_pressed = (GetAsyncKeyState(0x10) as i16) < 0; // VK_SHIFT
        let win_pressed = (GetAsyncKeyState(0x5B) as i16) < 0 || (GetAsyncKeyState(0x5C) as i16) < 0; // VK_LWIN / VK_RWIN

        match evaluate_keystroke(kbd.vkCode, kbd.flags.0, ctrl_pressed, shift_pressed, win_pressed) {
            KeyAction::Suppress => {
                // Drop the hotkey: return 1 so Windows does not process it
                return LRESULT(1);
            }
            KeyAction::EmergencyOverride => {
                EMERGENCY_OVERRIDE_TRIGGERED.store(true, Ordering::SeqCst);
                eprintln!("[CITADEL CLIENT] PROCTOR EMERGENCY OVERRIDE KEY COMBINATION DETECTED.");
                return LRESULT(1);
            }
            KeyAction::Allow => {
                // Pass through normal keystrokes
            }
        }
    }

    CallNextHookEx(HHOOK(std::ptr::null_mut()), code, wparam, lparam)
}

/// RAII handle to the active low-level hotkey suppression hook.
pub struct HotkeyLockHandle {
    thread_id: u32,
    join_handle: Option<JoinHandle<()>>,
}

impl HotkeyLockHandle {
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

impl Drop for HotkeyLockHandle {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Installs the system-wide hotkey suppression hook on a dedicated background thread.
pub fn install_hotkey_lock() -> Result<HotkeyLockHandle, String> {
    reset_emergency_override();
    let (tx, rx) = mpsc::channel::<Result<u32, String>>();

    let join_handle = thread::spawn(move || {
        let thread_id = unsafe { GetCurrentThreadId() };
        let hhook = unsafe {
            SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(hotkey_hook_proc),
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

        let mut msg = MSG::default();
        while unsafe { GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0).as_bool() } {
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        unsafe {
            let _ = UnhookWindowsHookEx(hhook);
        }
    });

    let thread_id = rx
        .recv()
        .map_err(|e| format!("Failed to initialize hotkey hook thread: {}", e))??;

    Ok(HotkeyLockHandle {
        thread_id,
        join_handle: Some(join_handle),
    })
}
