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

#[tokio::test]
async fn test_roster_authentication_and_login_validation() {
    let test_dir = std::env::temp_dir().join(format!("citadel_test_roster_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)));
    let app = build_app_with_state(AppState::new_with_dir(test_dir.clone()));

    // 1. Upload Roster with 2 candidates using OA-style email identifiers
    let roster_payload = json!({
        "exam_id": "OA-EXAM-2026",
        "candidates": [
            {
                "email": "alex.mercer@univ.edu",
                "name": "Alex Mercer",
                "section": "A",
                "allowed": true
            },
            {
                "email": "sarah.connor@univ.edu",
                "name": "Sarah Connor",
                "section": "B",
                "allowed": false
            }
        ]
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/roster/upload?key=citadel-recruiter-key-2026")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&roster_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 2. Attempt login with INVALID PASSCODE -> 401 UNAUTHORIZED
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "email": "alex.mercer@univ.edu",
                        "passcode": "WRONG_PASSCODE"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let err_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(err_json["error"], "INVALID_PASSCODE");

    // 3. Attempt login with unlisted email address -> 403 FORBIDDEN
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "email": "intruder@evil.org",
                        "passcode": "CITADEL2026"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let err_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(err_json["error"], "ROSTER_NOT_FOUND");

    // 4. Attempt login with revoked student -> 403 FORBIDDEN
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "email": "sarah.connor@univ.edu",
                        "passcode": "CITADEL2026"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let err_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(err_json["error"], "ACCESS_REVOKED");

    // 5. Valid Candidate Login with correct Email + Passcode -> 200 OK
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "email": "alex.mercer@univ.edu",
                        "passcode": "CITADEL2026"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let login_resp: CandidateLoginResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(login_resp.status, "created");
    assert_eq!(login_resp.candidate_id, "alex.mercer@univ.edu");
    assert_eq!(login_resp.name, "Alex Mercer");
    assert!(login_resp.resume_state.is_none());

    // Cleanup
    let _ = std::fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_state_sync_and_session_resumption() {
    let test_dir = std::env::temp_dir().join(format!("citadel_test_sync_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)));
    let app = build_app_with_state(AppState::new_with_dir(test_dir.clone()));

    let candidate_email = "devin.miller@tech.org";

    // 1. Candidate logs in with email + passcode
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "email": candidate_email,
                        "name": "Devin Miller",
                        "passcode": "CITADEL2026"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 2. Candidate types code and syncs state
    let sync_payload = json!({
        "candidate_id": candidate_email,
        "active_question_id": "q1-two-sum",
        "active_language": "python",
        "remaining_seconds": 4500,
        "code_store": {
            "q1-two-sum": {
                "python": "def two_sum(nums, target): return [0, 1]"
            },
            "q2-three-sum": {
                "python": "def three_sum(nums): return [[-1, 0, 1]]"
            }
        },
        "state_version": 5
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/state/sync")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&sync_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 3. Proctor extends time by 10 minutes (600s)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/admin/candidates/{}/extend-time?key=citadel-recruiter-key-2026", candidate_email))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "extra_seconds": 600 }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 4. Candidate workstation crashes and restarts -> Re-logs in with email + passcode!
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "email": candidate_email,
                        "passcode": "CITADEL2026"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let login_resp: CandidateLoginResponse = serde_json::from_slice(&body).unwrap();

    assert_eq!(login_resp.status, "resumed");
    assert!(login_resp.resume_state.is_some());
    let resume = login_resp.resume_state.unwrap();

    assert_eq!(resume.active_question_id, "q1-two-sum");
    assert_eq!(resume.active_language, "python");
    assert!(resume.remaining_seconds >= 5100);
    assert_eq!(
        resume.code_store.get("q1-two-sum").unwrap().get("python").unwrap(),
        "def two_sum(nums, target): return [0, 1]"
    );
    assert_eq!(
        resume.code_store.get("q2-three-sum").unwrap().get("python").unwrap(),
        "def three_sum(nums): return [[-1, 0, 1]]"
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_server_restart_crash_recovery_from_disk() {
    let test_dir = std::env::temp_dir().join(format!("citadel_test_crash_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)));
    let candidate_email = "elena.fisher@adventure.net";

    // SERVER INSTANCE 1
    {
        let app1 = build_app_with_state(AppState::new_with_dir(test_dir.clone()));

        // 1. Upload roster
        let _ = app1
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/admin/roster/upload?key=citadel-recruiter-key-2026")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(json!({
                        "exam_id": "RECOVERY-EXAM",
                        "candidates": [
                            {"email": candidate_email, "name": "Elena Fisher", "allowed": true}
                        ]
                    }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // 2. Candidate logs in and works on exam
        let _ = app1
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(json!({
                        "email": candidate_email,
                        "passcode": "CITADEL2026"
                    }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // 3. Candidate syncs their progress
        let _ = app1
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/state/sync")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(json!({
                        "candidate_id": candidate_email,
                        "active_question_id": "q3-max-subarray",
                        "active_language": "cpp",
                        "remaining_seconds": 3333,
                        "code_store": {
                            "q3-max-subarray": { "cpp": "#include <iostream>\nint main() { return 0; }" }
                        }
                    }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // SERVER INSTANCE 1 SHUTS DOWN
    }

    // SERVER INSTANCE 2 STARTS UP (re-reading from disk)
    {
        let app2 = build_app_with_state(AppState::new_with_dir(test_dir.clone()));

        // Candidate reconnects to the newly started server
        let res = app2
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(json!({
                        "email": candidate_email,
                        "passcode": "CITADEL2026"
                    }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let body = res.into_body().collect().await.unwrap().to_bytes();
        let login_resp: CandidateLoginResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(login_resp.status, "resumed");
        assert!(login_resp.resume_state.is_some());
        let resume = login_resp.resume_state.unwrap();

        // Complete state survived the server reboot!
        assert_eq!(resume.active_question_id, "q3-max-subarray");
        assert_eq!(resume.active_language, "cpp");
        assert_eq!(
            resume.code_store.get("q3-max-subarray").unwrap().get("cpp").unwrap(),
            "#include <iostream>\nint main() { return 0; }"
        );
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&test_dir);
}
