use guard_svc::llm_detect::{is_capture_excluded_affinity, ExcludedWindowViolation};

#[test]
fn test_affinity_allowed_cases() {
    // Normal windows have affinity WDA_NONE (0)
    assert!(!is_capture_excluded_affinity(0));
    // WDA_MONITOR (1) is standard display monitor affinity, not capture exclusion
    assert!(!is_capture_excluded_affinity(1));
    // Arbitrary other flags without 0x11
    assert!(!is_capture_excluded_affinity(0x02));
    assert!(!is_capture_excluded_affinity(0x10));
}

#[test]
fn test_affinity_violation_cases() {
    // WDA_EXCLUDEFROMCAPTURE is exactly 0x11 (17)
    assert!(is_capture_excluded_affinity(0x11));
    assert!(is_capture_excluded_affinity(17));
    // Composite flag: WDA_EXCLUDEFROMCAPTURE with future additional Windows bits
    assert!(is_capture_excluded_affinity(0x11 | 0x04));
    assert!(is_capture_excluded_affinity(0x11 | 0x80));
}

#[test]
fn test_excluded_window_log_format() {
    let violation = ExcludedWindowViolation {
        pid: 4321,
        title: "Cheat Overlay Assistant".to_string(),
        affinity: 0x11,
        timestamp: "2026-09-25T12:00:00Z".to_string(),
    };

    let log_line = violation.to_log_line();
    assert!(log_line.starts_with("VIOLATION capture_exclusion"));
    assert!(log_line.contains("pid=4321"));
    assert!(log_line.contains("affinity=0x11"));
    assert!(log_line.contains("title=\"Cheat Overlay Assistant\""));
    assert!(log_line.ends_with("2026-09-25T12:00:00Z"));
}
