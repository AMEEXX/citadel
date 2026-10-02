use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use tower::ServiceExt;

use citadel_server::{
    api::{is_mobile_or_tablet_user_agent, AppState},
    build_app_with_state,
    persistence::RosterEntry,
    ui::{render_gatekeeper_html, render_mobile_blocked_html},
};

fn create_test_state(is_prod: bool) -> (AppState, PathBuf) {
    let test_dir = std::env::temp_dir().join(format!(
        "citadel_test_device_detect_{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    let state = AppState::new_with_dir(test_dir.clone());
    state.is_production.store(is_prod, Ordering::SeqCst);
    (state, test_dir)
}

#[test]
fn test_is_mobile_or_tablet_user_agent_detection() {
    // 1. Mobile Smartphones
    let iphone_ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 16_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.5 Mobile/15E148 Safari/604.1";
    let android_phone_ua = "Mozilla/5.0 (Linux; Android 13; SM-S901B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36";
    let opera_mini_ua = "Opera/9.80 (Android; Opera Mini/36.2.2254/119.132; U; id) Presto/2.12.423 Version/12.16";
    let blackberry_ua = "Mozilla/5.0 (BlackBerry; U; BlackBerry 9800; en) AppleWebKit/534.1+ (KHTML, like Gecko) Version/6.0.0.337 Mobile Safari/534.1+";

    assert!(is_mobile_or_tablet_user_agent(iphone_ua));
    assert!(is_mobile_or_tablet_user_agent(android_phone_ua));
    assert!(is_mobile_or_tablet_user_agent(opera_mini_ua));
    assert!(is_mobile_or_tablet_user_agent(blackberry_ua));

    // 2. Tablets
    let ipad_ua = "Mozilla/5.0 (iPad; CPU OS 16_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/114.0.5735.99 Mobile/15E148 Safari/604.1";
    let android_tablet_ua = "Mozilla/5.0 (Linux; Android 12; SM-X906C Build/SP1A.210812.016) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/103.0.5060.71 Safari/537.36";
    let kindle_ua = "Mozilla/5.0 (Linux; U; Android 4.4.3; en-us; KFTHWI Build/KTU84M) AppleWebKit/537.36 (KHTML, like Gecko) Silk/3.68 like Chrome/39.0.2171.93 Safari/537.36";

    assert!(is_mobile_or_tablet_user_agent(ipad_ua));
    assert!(is_mobile_or_tablet_user_agent(android_tablet_ua));
    assert!(is_mobile_or_tablet_user_agent(kindle_ua));

    // 3. Laptops & Desktop Workstations (Must NOT be flagged as mobile)
    let windows_laptop_chrome = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
    let windows_laptop_edge = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 Edg/120.0.0.0";
    let mac_laptop_safari = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";
    let linux_desktop_firefox = "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/119.0";
    let citadel_kiosk_client = "Mozilla/5.0 CitadelSecurityCore/0.2.0 Win32/x64";

    assert!(!is_mobile_or_tablet_user_agent(windows_laptop_chrome));
    assert!(!is_mobile_or_tablet_user_agent(windows_laptop_edge));
    assert!(!is_mobile_or_tablet_user_agent(mac_laptop_safari));
    assert!(!is_mobile_or_tablet_user_agent(linux_desktop_firefox));
    assert!(!is_mobile_or_tablet_user_agent(citadel_kiosk_client));
}

#[test]
fn test_ui_gatekeeper_and_mobile_blocked_rendering() {
    // 1. Mobile Blocked page contains the exact required prompt
    let blocked_html = render_mobile_blocked_html();
    assert!(blocked_html.contains("This exam needs to be taken from a laptop"));
    assert!(blocked_html.contains("Laptop Required for Assessment"));
    assert!(blocked_html.contains("DEVICE_DISALLOWED: NON-LAPTOP DEVICE"));

    // 2. Gatekeeper in Production with Mobile UA sets warning banner visible
    let gk_prod_mobile = render_gatekeeper_html(true, true);
    assert!(gk_prod_mobile.contains("This exam needs to be taken from a laptop"));
    assert!(gk_prod_mobile.contains(r#"id="mobile-warning-banner" class="mobile-warning" style="display: block;"#));
    assert!(!gk_prod_mobile.contains("Launch Web Assessment Directly (Testing Mode)"));

    // 3. Gatekeeper in Testing Mode allows web launch directly
    let gk_test_mobile = render_gatekeeper_html(false, true);
    assert!(gk_test_mobile.contains("Launch Web Assessment Directly (Testing Mode)"));
}

#[tokio::test]
async fn test_production_mode_strictly_blocks_mobile_and_tablet_logins() {
    let (state, _temp_dir) = create_test_state(true); // Production Mode

    // Whitelist candidate
    {
        let mut roster = state.roster.write().unwrap();
        roster.candidates.push(RosterEntry {
            email: "enrolled.student@college.edu".to_string(),
            roll_number: Some("COL-2026-001".to_string()),
            name: "Enrolled Student".to_string(),
            section: None,
            allowed: true,
        });
    }

    let app = build_app_with_state(state.clone());

    // 1. Attempt candidate login with iPhone User-Agent in Production Mode -> Blocked with 403 Forbidden
    let iphone_ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 16_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.5 Mobile/15E148 Safari/604.1";
    let login_body = serde_json::json!({
        "email": "enrolled.student@college.edu",
        "passcode": "CITADEL2026"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::USER_AGENT, iphone_ua)
        .body(Body::from(serde_json::to_string(&login_body).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"], "DEVICE_DISALLOWED");
    assert!(body["message"].as_str().unwrap().contains("This exam needs to be taken from a laptop"));

    // 2. Attempt candidate login with Android Phone User-Agent -> Blocked with 403 Forbidden
    let android_ua = "Mozilla/5.0 (Linux; Android 13; SM-S901B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36";
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::USER_AGENT, android_ua)
        .body(Body::from(serde_json::to_string(&login_body).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // 3. Attempt candidate login with Windows Laptop User-Agent -> Accepted with 200 OK
    let laptop_ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::USER_AGENT, laptop_ua)
        .body(Body::from(serde_json::to_string(&login_body).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_testing_mode_permits_all_devices_including_mobile() {
    let (state, _temp_dir) = create_test_state(false); // Testing Mode

    // Whitelist candidate
    {
        let mut roster = state.roster.write().unwrap();
        roster.candidates.push(RosterEntry {
            email: "qa.tester@college.edu".to_string(),
            roll_number: Some("COL-2026-QA".to_string()),
            name: "QA Tester".to_string(),
            section: None,
            allowed: true,
        });
    }

    let app = build_app_with_state(state.clone());

    // Candidate login with iPhone User-Agent in Testing Mode -> Allowed 200 OK!
    let iphone_ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 16_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.5 Mobile/15E148 Safari/604.1";
    let login_body = serde_json::json!({
        "email": "qa.tester@college.edu",
        "passcode": "CITADEL2026"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::USER_AGENT, iphone_ua)
        .body(Body::from(serde_json::to_string(&login_body).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(body["status"] == "created" || body["status"] == "resumed");
    assert_eq!(body["candidate_id"], "qa.tester@college.edu");
}

#[tokio::test]
async fn test_production_mode_exam_endpoint_serves_mobile_blocked_page() {
    let (state, _temp_dir) = create_test_state(true); // Production Mode
    let app = build_app_with_state(state.clone());

    let iphone_ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 16_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.5 Mobile/15E148 Safari/604.1";
    let req = Request::builder()
        .method("GET")
        .uri("/exam")
        .header(header::USER_AGENT, iphone_ua)
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    // In production mode with mobile UA, returns 403 Forbidden with laptop required notice
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body_str = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(body_str.contains("This exam needs to be taken from a laptop"));
}
