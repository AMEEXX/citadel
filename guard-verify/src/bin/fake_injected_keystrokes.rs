use std::thread;
use std::time::Duration;
use guard_svc::llm_detect::{
    install_keyboard_hook, is_injected_keystroke_flag, KeystrokeViolation,
};

fn main() {
    println!("============================================================");
    println!("CITADEL M5 Functional Test: Synthetic Keystroke Injection Hook");
    println!("============================================================");

    println!("[1/3] Verifying flag-level synthetic injection classification...");
    assert!(!is_injected_keystroke_flag(0x00), "Hardware key-down must be allowed");
    assert!(!is_injected_keystroke_flag(0x01), "Hardware extended key must be allowed");
    assert!(is_injected_keystroke_flag(0x10), "LLKHF_INJECTED must trigger violation");
    assert!(is_injected_keystroke_flag(0x11), "LLKHF_INJECTED + extended must trigger violation");

    println!("[2/3] Installing low-level keyboard hook (WH_KEYBOARD_LL)...");
    let hook_handle = install_keyboard_hook().expect("Failed to install WH_KEYBOARD_LL hook");
    thread::sleep(Duration::from_millis(200));

    let violation = KeystrokeViolation {
        vk_code: 65,
        timestamp: "2026-09-25T12:00:00Z".to_string(),
    };
    println!("[3/3] Checking privacy-compliant violation logging...");
    let log_line = violation.to_log_line();
    println!("      Generated log line: {}", log_line);
    assert_eq!(log_line, "VIOLATION input_injection vk_code=65 2026-09-25T12:00:00Z");
    assert!(!log_line.contains("'A'"));

    println!("[CLEANUP] Stopping keyboard hook and message pump...");
    hook_handle.stop();

    println!("============================================================");
    println!("M5 FUNCTIONAL TEST PASSED: WH_KEYBOARD_LL verified!");
    println!("============================================================");
}
