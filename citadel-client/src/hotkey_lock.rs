//! CITADEL Client Module: System Hotkey Suppression
//!
//! Intercepts and drops escape hotkeys (Alt+Tab, Win Key, Ctrl+Esc, Alt+F4)
//! to prevent the candidate from leaving the kiosk assessment window.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::{GetCurrentThreadId};
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
/// Distinguishes between normal coding inputs (Tab, Shift, Letters, Digits)
/// and system breakout combinations.
pub fn evaluate_keystroke(
    vk: u32,
    flags: u32,
    ctrl_pressed: bool,
    shift_pressed: bool,
) -> KeyAction {
    let alt_down = (flags & 0x20) != 0;

    // Proctor Emergency Override: Ctrl + Shift + Alt + F12 (VK 0x7B)
    if vk == 0x7B && ctrl_pressed && shift_pressed && alt_down {
        return KeyAction::EmergencyOverride;
    }

    // 1. Suppress Windows Keys (LWIN: 0x5B, RWIN: 0x5C)
    if vk == 0x5B || vk == 0x5C {
        return KeyAction::Suppress;
    }

    // 2. Suppress Alt + Tab (VK_TAB: 0x09 with Alt)
    if vk == 0x09 && alt_down {
        return KeyAction::Suppress;
    }

    // 3. Suppress Alt + Escape (VK_ESCAPE: 0x1B with Alt)
    if vk == 0x1B && alt_down {
        return KeyAction::Suppress;
    }

    // 4. Suppress Ctrl + Escape (VK_ESCAPE: 0x1B with Ctrl)
    if vk == 0x1B && ctrl_pressed {
        return KeyAction::Suppress;
    }

    // 5. Suppress Alt + F4 (VK_F4: 0x73 with Alt)
    if vk == 0x73 && alt_down {
        return KeyAction::Suppress;
    }

    // Normal typing, Enter, Backspace, Arrow keys, and code editor Tab are Allowed!
    KeyAction::Allow
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

        match evaluate_keystroke(kbd.vkCode, kbd.flags.0, ctrl_pressed, shift_pressed) {
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
