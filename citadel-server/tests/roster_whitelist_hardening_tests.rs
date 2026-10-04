use std::path::PathBuf;
use std::sync::atomic::Ordering;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

use citadel_server::{
    api::{AppState, CandidateLoginResponse},
    build_app_with_state,
};

fn create_test_state(is_prod: bool) -> (AppState, PathBuf) {
    let test_dir = std::env::temp_dir().join(format!(
        "citadel_test_roster_harden_{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    let state = AppState::new_with_dir(test_dir.clone());
    state.is_production.store(is_prod, Ordering::SeqCst);
    (state, test_dir)
}

#[tokio::test]
async fn test_testing_mode_strict_roster_whitelisting() {
    let (state, temp_dir) = create_test_state(false); // Testing Mode
    let app = build_app_with_state(state.clone());

    // Make exam live
    {
        let mut live = state.exam_live.write().unwrap();
        live.is_live = true;
        live.started_at = Some(chrono::Utc::now().to_rfc3339());
    }

    // 1. With EMPTY roster -> 403 ROSTER_EMPTY
    let res = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "email": "random_student_123",
                "passcode": "CITADEL2026"
            }).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let err: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(err["error"], "ROSTER_EMPTY");

    // 2. Upload roster with 2 students
    let roster_payload = json!({
        "exam_id": "TEST-EXAM-2026",
        "candidates": [
            {
                "email": "whitelisted.student@college.edu",
                "roll_number": "2026-CS-101",
                "name": "Whitelisted Student",
                "section": "A",
                "allowed": true
            },
            {
                "email": "revoked.student@college.edu",
                "roll_number": "2026-CS-102",
                "name": "Revoked Student",
                "section": "B",
                "allowed": false
            }
        ]
    });
    let up_res = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/admin/roster/upload?key=citadel-recruiter-key-2026")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&roster_payload).unwrap()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(up_res.status(), StatusCode::OK);

    // 3. Random unlisted ID -> 403 ROSTER_NOT_FOUND
    let res = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "email": "random_intruder_999",
                "passcode": "CITADEL2026"
            }).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let err: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(err["error"], "ROSTER_NOT_FOUND");

    // 4. Revoked candidate -> 403 ACCESS_REVOKED
    let res = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "email": "revoked.student@college.edu",
                "passcode": "CITADEL2026"
            }).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let err: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(err["error"], "ACCESS_REVOKED");

    // 5. Whitelisted candidate with correct credentials -> 200 OK
    let res = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "email": "whitelisted.student@college.edu",
                "passcode": "CITADEL2026"
            }).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let login: CandidateLoginResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(login.name, "Whitelisted Student");
    assert_eq!(login.candidate_id, "whitelisted.student@college.edu");

    // 6. Direct heartbeat from random unlisted candidate -> 403 FORBIDDEN
    let hb_res = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/integrity/heartbeat")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "candidate_id": "unlisted_hacker_404",
                "active_question": 1,
                "is_window_focused": true
            }).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(hb_res.status(), StatusCode::FORBIDDEN);

    // 7. Direct submission from random unlisted candidate -> 403 FORBIDDEN
    let sub_res = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/submissions")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "candidate_id": "unlisted_hacker_404",
                "question_id": "q1-two-sum",
                "language": "python",
                "source_code": "def twoSum(nums, target): return [0,1]",
                "is_sample_run": true
            }).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(sub_res.status(), StatusCode::FORBIDDEN);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_production_mode_strict_roster_whitelisting() {
    let (state, temp_dir) = create_test_state(true); // Production Mode
    let app = build_app_with_state(state.clone());

    // Make exam live
    {
        let mut live = state.exam_live.write().unwrap();
        live.is_live = true;
        live.started_at = Some(chrono::Utc::now().to_rfc3339());
    }

    // 1. Upload roster
    let roster_payload = json!({
        "exam_id": "PROD-EXAM-2026",
        "candidates": [
            {
                "email": "prod.cand@university.edu",
                "roll_number": "PROD-001",
                "name": "Prod Candidate",
                "allowed": true
            }
        ]
    });
    let up_res = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/admin/roster/upload?key=citadel-recruiter-key-2026")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&roster_payload).unwrap()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(up_res.status(), StatusCode::OK);

    // 2. Random Candidate ID in Production Mode -> 403 ROSTER_NOT_FOUND
    let res = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "email": "random_prod_intruder",
                "passcode": "CITADEL2026"
            }).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let err: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(err["error"], "ROSTER_NOT_FOUND");

    // 3. Whitelisted Candidate in Production Mode -> 200 OK
    let res = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "email": "prod.cand@university.edu",
                "passcode": "CITADEL2026"
            }).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let login: CandidateLoginResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(login.is_production, true);
    assert!(!login.session_token.is_empty());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_case_insensitive_and_roll_number_roster_resolution() {
    let (state, temp_dir) = create_test_state(false);
    let app = build_app_with_state(state.clone());

    let roster_payload = json!({
        "exam_id": "CASE-TEST",
        "candidates": [
            {
                "email": "student.alpha@univ.ac.in",
                "roll_number": "2026-BCS-042",
                "name": "Alpha Student",
                "allowed": true
            }
        ]
    });
    let _ = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/admin/roster/upload?key=citadel-recruiter-key-2026")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(&roster_payload).unwrap()))
            .unwrap(),
    ).await.unwrap();

    // 1. Match by Upper-case Email
    let res = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "email": "STUDENT.ALPHA@UNIV.AC.IN",
                "passcode": "CITADEL2026"
            }).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 2. Match by Roll Number
    let res2 = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "roll_number": "2026-bcs-042",
                "passcode": "CITADEL2026"
            }).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(res2.status(), StatusCode::OK);

    // 3. Match by email username prefix
    let res3 = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "email": "student.alpha",
                "passcode": "CITADEL2026"
            }).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(res3.status(), StatusCode::OK);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_dynamic_roster_mutation_and_immediate_enforcement() {
    let (state, temp_dir) = create_test_state(false);
    let app = build_app_with_state(state.clone());

    // 1. Initial login fails -> 403 ROSTER_EMPTY
    let res = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "email": "new.recruit@defense.gov",
                "passcode": "CITADEL2026"
            }).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // 2. Proctor dynamically adds candidate
    let add_res = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/admin/roster/add?key=citadel-recruiter-key-2026")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "email": "new.recruit@defense.gov",
                "name": "New Recruit",
                "allowed": true
            }).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(add_res.status(), StatusCode::OK);

    // 3. Candidate now logs in successfully -> 200 OK
    let res2 = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "email": "new.recruit@defense.gov",
                "passcode": "CITADEL2026"
            }).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(res2.status(), StatusCode::OK);

    // 4. Proctor revokes access -> immediate 403 ACCESS_REVOKED
    let revoke_res = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/admin/roster/add?key=citadel-recruiter-key-2026")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "email": "new.recruit@defense.gov",
                "name": "New Recruit",
                "allowed": false
            }).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(revoke_res.status(), StatusCode::OK);

    let res3 = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json!({
                "email": "new.recruit@defense.gov",
                "passcode": "CITADEL2026"
            }).to_string()))
            .unwrap(),
    ).await.unwrap();
    assert_eq!(res3.status(), StatusCode::FORBIDDEN);
    let body = res3.into_body().collect().await.unwrap().to_bytes();
    let err: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(err["error"], "ACCESS_REVOKED");

    let _ = std::fs::remove_dir_all(&temp_dir);
}
