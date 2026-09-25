use guard_svc::format_log_line;

fn matches_pattern(s: &str) -> bool {
    // pattern: ^SERVICE (STARTED|STOPPED) \d{4}-\d{2}-\d{2}T
    let rest = if let Some(stripped) = s.strip_prefix("SERVICE STARTED ") {
        stripped
    } else if let Some(stripped) = s.strip_prefix("SERVICE STOPPED ") {
        stripped
    } else {
        return false;
    };

    let bytes = rest.as_bytes();
    if bytes.len() < 11 {
        return false;
    }

    bytes[0..4].iter().all(|b| b.is_ascii_digit())
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(|b| b.is_ascii_digit())
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(|b| b.is_ascii_digit())
        && bytes[10] == b'T'
}

#[test]
fn test_log_format_started() {
    let line = format_log_line("STARTED");
    assert!(
        matches_pattern(&line),
        "Expected '{}' to match pattern ^SERVICE (STARTED|STOPPED) \\d{{4}}-\\d{{2}}-\\d{{2}}T",
        line
    );
}

#[test]
fn test_log_format_stopped() {
    let line = format_log_line("STOPPED");
    assert!(
        matches_pattern(&line),
        "Expected '{}' to match pattern ^SERVICE (STARTED|STOPPED) \\d{{4}}-\\d{{2}}-\\d{{2}}T",
        line
    );
}
