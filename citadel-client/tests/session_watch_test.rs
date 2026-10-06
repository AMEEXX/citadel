use std::io::Cursor;
use citadel_client::session_watch::{parse_session_control_exit, read_http_response};

#[test]
fn test_read_http_response_keep_alive() {
    let raw_http = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 51\r\n\r\n{\"should_exit\":false,\"reason\":\"\",\"status\":\"Active\"}";
    let mut cursor = Cursor::new(raw_http);
    let body = read_http_response(&mut cursor).expect("Failed to parse HTTP keep-alive response");
    assert_eq!(body, "{\"should_exit\":false,\"reason\":\"\",\"status\":\"Active\"}");
}

#[test]
fn test_parse_session_control_active() {
    let body = "{\"should_exit\":false,\"reason\":\"\",\"status\":\"Active\"}";
    let (should_exit, status) = parse_session_control_exit(body);
    assert!(!should_exit);
    assert_eq!(status, "Active");
}

#[test]
fn test_parse_session_control_disqualified() {
    let body = "{\"should_exit\":true,\"reason\":\"Candidate disqualified\",\"status\":\"Disqualified\"}";
    let (should_exit, status) = parse_session_control_exit(body);
    assert!(should_exit);
    assert_eq!(status, "Disqualified");
}

#[test]
fn test_parse_session_control_force_restore() {
    let body = "{\"should_exit\":true,\"reason\":\"Workstation restore requested via server\",\"status\":\"Restoring\"}";
    let (should_exit, status) = parse_session_control_exit(body);
    assert!(should_exit);
    assert_eq!(status, "Restoring");
}
