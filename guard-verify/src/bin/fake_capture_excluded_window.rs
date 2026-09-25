use windows::core::w;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GetWindowDisplayAffinity, SetWindowDisplayAffinity,
    ShowWindow, SW_SHOW, WDA_EXCLUDEFROMCAPTURE, WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW,
};
use guard_svc::llm_detect::{is_capture_excluded_affinity, ExcludedWindowViolation};

fn main() {
    println!("============================================================");
    println!("CITADEL M4 Functional Test: Capture-Exclusion Window Detector");
    println!("============================================================");

    unsafe {
        println!("[1/4] Spawning test window with WDA_EXCLUDEFROMCAPTURE (0x11)...");
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("CITADEL Rogue Cheat Overlay"),
            WS_OVERLAPPEDWINDOW,
            100, 100, 300, 200,
            HWND(std::ptr::null_mut()),
            None,
            None,
            None,
        ).expect("Failed to create test window");

        let _ = ShowWindow(hwnd, SW_SHOW);
        SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)
            .expect("Failed to set WDA_EXCLUDEFROMCAPTURE");

        println!("[2/4] Verifying window display affinity via kernel API...");
        let mut affinity = 0u32;
        GetWindowDisplayAffinity(hwnd, &mut affinity)
            .expect("Failed to read window display affinity");

        println!("      Detected Window Affinity: {:#x}", affinity);
        assert_eq!(affinity, 0x11, "Expected affinity 0x11 (WDA_EXCLUDEFROMCAPTURE)");
        assert!(is_capture_excluded_affinity(affinity), "Expected is_capture_excluded_affinity to be true");

        let violation = ExcludedWindowViolation {
            pid: std::process::id(),
            title: "CITADEL Rogue Cheat Overlay".to_string(),
            affinity,
            timestamp: "2026-09-25T12:00:00Z".to_string(),
        };

        let log_line = violation.to_log_line();
        println!("[3/4] Formatted Violation Log: {}", log_line);
        assert!(log_line.starts_with("VIOLATION capture_exclusion"));
        assert!(log_line.contains("affinity=0x11"));

        println!("[4/4] Destroying test window and verifying cleanup...");
        let _ = DestroyWindow(hwnd);

        let mut post_affinity = 0u32;
        let post_res = GetWindowDisplayAffinity(hwnd, &mut post_affinity);
        println!("      Post-destruction GetWindowDisplayAffinity result: {:?}", post_res);
        assert!(post_res.is_err(), "Destroyed window should no longer return valid affinity");

        println!("============================================================");
        println!("M4 FUNCTIONAL TEST PASSED: Capture exclusion identified & cleared!");
        println!("============================================================");
    }
}
