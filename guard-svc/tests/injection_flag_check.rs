use guard_svc::llm_detect::{is_injected_keystroke_flag, KeystrokeViolation};

#[test]
fn test_injection_flag_hardware_keystrokes() {
    // Normal genuine hardware input has LLKHF_INJECTED (0x10) cleared
    assert!(!is_injected_keystroke_flag(0x00));
    // Hardware input with extended key flag (e.g. arrow keys)
    assert!(!is_injected_keystroke_flag(0x01));
    // Hardware input with key-up flag
    assert!(!is_injected_keystroke_flag(0x80));
    // Hardware input with alt-down flag
    assert!(!is_injected_keystroke_flag(0x20));
}

#[test]
fn test_injection_flag_synthetic_injected() {
    // Basic SendInput injected flag
    assert!(is_injected_keystroke_flag(0x10));
    // Injected + extended key flag
    assert!(is_injected_keystroke_flag(0x11));
    // Injected + key-up flag
    assert!(is_injected_keystroke_flag(0x90));
    // Injected + alt-down flag
    assert!(is_injected_keystroke_flag(0x30));
}

#[test]
fn test_injection_privacy_log_format() {
    // Document 10 / Document 17 Privacy guarantee:
    // Only vk_code and timestamp are logged, never ASCII or typed characters.
    let violation = KeystrokeViolation {
        vk_code: 65, // Virtual Key for 'A'
        timestamp: "2026-09-25T12:00:00Z".to_string(),
    };

    let log_line = violation.to_log_line();
    assert_eq!(
        log_line,
        "VIOLATION input_injection vk_code=65 2026-09-25T12:00:00Z"
    );
    // Explicitly verify no character conversion appears in log line
    assert!(!log_line.contains("'A'"));
    assert!(!log_line.contains("\"A\""));
}
