use citadel_client::hotkey_lock::{evaluate_keystroke, KeyAction};

#[test]
fn test_alt_tab_suppressed() {
    // VK_TAB = 0x09, LLKHF_ALTDOWN = 0x20
    let action = evaluate_keystroke(0x09, 0x20, false, false, false);
    assert_eq!(action, KeyAction::Suppress);
}

#[test]
fn test_normal_tab_in_editor_allowed() {
    // Normal Tab key pressed while writing code has alt_down = 0, win_pressed = false
    let action = evaluate_keystroke(0x09, 0x00, false, false, false);
    assert_eq!(action, KeyAction::Allow);
}

#[test]
fn test_windows_keys_and_combinations_suppressed() {
    // VK_LWIN = 0x5B, VK_RWIN = 0x5C
    assert_eq!(evaluate_keystroke(0x5B, 0x00, false, false, false), KeyAction::Suppress);
    assert_eq!(evaluate_keystroke(0x5C, 0x00, false, false, false), KeyAction::Suppress);

    // Win + Tab (0x09 with win_pressed = true)
    assert_eq!(evaluate_keystroke(0x09, 0x00, false, false, true), KeyAction::Suppress);
    // Win + D (0x44 with win_pressed = true)
    assert_eq!(evaluate_keystroke(0x44, 0x00, false, false, true), KeyAction::Suppress);
    // Win + V (Clipboard History)
    assert_eq!(evaluate_keystroke(0x56, 0x00, false, false, true), KeyAction::Suppress);
}

#[test]
fn test_alt_shortcuts_suppressed() {
    // Alt + F4 (0x73)
    assert_eq!(evaluate_keystroke(0x73, 0x20, false, false, false), KeyAction::Suppress);
    // Alt + Space (0x20)
    assert_eq!(evaluate_keystroke(0x20, 0x20, false, false, false), KeyAction::Suppress);
    // Alt + Esc (0x1B)
    assert_eq!(evaluate_keystroke(0x1B, 0x20, false, false, false), KeyAction::Suppress);
}

#[test]
fn test_ctrl_shortcuts_suppressed() {
    // Ctrl + Esc (0x1B)
    assert_eq!(evaluate_keystroke(0x1B, 0x00, true, false, false), KeyAction::Suppress);
    // Ctrl + Shift + Esc (Task Manager)
    assert_eq!(evaluate_keystroke(0x1B, 0x00, true, true, false), KeyAction::Suppress);
    // Ctrl + T (New tab), Ctrl + W (Close tab), Ctrl + N (New window)
    assert_eq!(evaluate_keystroke(0x54, 0x00, true, false, false), KeyAction::Suppress);
    assert_eq!(evaluate_keystroke(0x57, 0x00, true, false, false), KeyAction::Suppress);
    assert_eq!(evaluate_keystroke(0x4E, 0x00, true, false, false), KeyAction::Suppress);
}

#[test]
fn test_print_screen_and_apps_suppressed() {
    // VK_SNAPSHOT = 0x2C
    assert_eq!(evaluate_keystroke(0x2C, 0x00, false, false, false), KeyAction::Suppress);
    // VK_APPS = 0x5D
    assert_eq!(evaluate_keystroke(0x5D, 0x00, false, false, false), KeyAction::Suppress);
}

#[test]
fn test_normal_typing_keys_allowed() {
    // 'A' = 0x41, 'Z' = 0x5A, '1' = 0x31, Enter = 0x0D, Space = 0x20, Backspace = 0x08
    assert_eq!(evaluate_keystroke(0x41, 0x00, false, false, false), KeyAction::Allow);
    assert_eq!(evaluate_keystroke(0x5A, 0x00, false, false, false), KeyAction::Allow);
    assert_eq!(evaluate_keystroke(0x31, 0x00, false, false, false), KeyAction::Allow);
    assert_eq!(evaluate_keystroke(0x0D, 0x00, false, false, false), KeyAction::Allow);
    assert_eq!(evaluate_keystroke(0x20, 0x00, false, false, false), KeyAction::Allow);
    assert_eq!(evaluate_keystroke(0x08, 0x00, false, false, false), KeyAction::Allow);
}

#[test]
fn test_proctor_emergency_override() {
    // F12 = 0x7B with Ctrl + Shift + Alt
    let action = evaluate_keystroke(0x7B, 0x20, true, true, false);
    assert_eq!(action, KeyAction::EmergencyOverride);

    // F12 without override keys is suppressed (prevents candidate opening devtools)
    let normal_f12 = evaluate_keystroke(0x7B, 0x00, false, false, false);
    assert_eq!(normal_f12, KeyAction::Suppress);
}
