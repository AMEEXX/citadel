use citadel_client::hotkey_lock::{evaluate_keystroke, KeyAction};

#[test]
fn test_alt_tab_suppressed() {
    // VK_TAB = 0x09, LLKHF_ALTDOWN = 0x20
    let action = evaluate_keystroke(0x09, 0x20, false, false);
    assert_eq!(action, KeyAction::Suppress);
}

#[test]
fn test_normal_tab_in_editor_allowed() {
    // Normal Tab key pressed while writing code has alt_down = 0
    let action = evaluate_keystroke(0x09, 0x00, false, false);
    assert_eq!(action, KeyAction::Allow);
}

#[test]
fn test_windows_keys_suppressed() {
    // VK_LWIN = 0x5B, VK_RWIN = 0x5C
    assert_eq!(evaluate_keystroke(0x5B, 0x00, false, false), KeyAction::Suppress);
    assert_eq!(evaluate_keystroke(0x5C, 0x00, false, false), KeyAction::Suppress);
}

#[test]
fn test_alt_f4_suppressed() {
    // VK_F4 = 0x73, LLKHF_ALTDOWN = 0x20
    let action = evaluate_keystroke(0x73, 0x20, false, false);
    assert_eq!(action, KeyAction::Suppress);
}

#[test]
fn test_ctrl_esc_and_alt_esc_suppressed() {
    // VK_ESCAPE = 0x1B
    assert_eq!(evaluate_keystroke(0x1B, 0x20, false, false), KeyAction::Suppress); // Alt + Esc
    assert_eq!(evaluate_keystroke(0x1B, 0x00, true, false), KeyAction::Suppress);  // Ctrl + Esc
}

#[test]
fn test_normal_typing_keys_allowed() {
    // 'A' = 0x41, 'Z' = 0x5A, '1' = 0x31, Enter = 0x0D, Space = 0x20, Backspace = 0x08
    assert_eq!(evaluate_keystroke(0x41, 0x00, false, false), KeyAction::Allow);
    assert_eq!(evaluate_keystroke(0x5A, 0x00, false, false), KeyAction::Allow);
    assert_eq!(evaluate_keystroke(0x31, 0x00, false, false), KeyAction::Allow);
    assert_eq!(evaluate_keystroke(0x0D, 0x00, false, false), KeyAction::Allow);
    assert_eq!(evaluate_keystroke(0x20, 0x00, false, false), KeyAction::Allow);
    assert_eq!(evaluate_keystroke(0x08, 0x00, false, false), KeyAction::Allow);
}

#[test]
fn test_proctor_emergency_override() {
    // F12 = 0x7B with Ctrl + Shift + Alt
    let action = evaluate_keystroke(0x7B, 0x20, true, true);
    assert_eq!(action, KeyAction::EmergencyOverride);

    // F12 without Alt should be Allowed
    let normal_f12 = evaluate_keystroke(0x7B, 0x00, false, false);
    assert_eq!(normal_f12, KeyAction::Allow);
}
