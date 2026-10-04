use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

use citadel_server::{
    api::{AppState, CandidateLoginResponse},
    build_app_with_state,
};

#[tokio::test]
async fn test_concurrent_state_sync_and_persistence_under_load() {
    let test_dir = std::env::temp_dir().join(format!("citadel_heavy_stress_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)));
    let app = Arc::new(build_app_with_state(AppState::new_with_dir(test_dir.clone())));

    const NUM_STUDENTS: usize = 30;

    // 1. Populate roster with 30 candidates using email addresses
    let mut roster_candidates = Vec::new();
    for i in 1..=NUM_STUDENTS {
        roster_candidates.push(json!({
            "email": format!("student_{:03}@assessment.citadel.org", i),
            "name": format!("Student Candidate {:03}", i),
            "section": if i % 2 == 0 { "Section-A" } else { "Section-B" },
            "allowed": true
        }));
    }

    let roster_payload = json!({
        "exam_id": "HEAVY-LOAD-TEST-2026",
        "candidates": roster_candidates
    });

    let res = (*app)
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

    // 2. Launch 30 concurrent tokio async tasks simulating 30 candidates taking the OA simultaneously
    let mut handles = Vec::new();

    for i in 1..=NUM_STUDENTS {
        let app_clone = Arc::clone(&app);
        let email = format!("student_{:03}@assessment.citadel.org", i);
        let name = format!("Student Candidate {:03}", i);

        let handle = tokio::spawn(async move {
            // Student logs in with Email + Passcode
            let res = (*app_clone)
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/v1/auth/login")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(json!({
                            "email": email,
                            "name": name,
                            "passcode": "CITADEL2026"
                        }).to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK);

            // Immediately simulate typing & continuous state syncing (5 cycles per student)
            for cycle in 1..=5 {
                let code_content = format!("// Student {} Cycle {}\nint ans = {};", i, cycle, cycle * 10);
                let sync_body = json!({
                    "candidate_id": email,
                    "active_question_id": format!("q-{:03}", (cycle % 3) + 1),
                    "active_language": "python",
                    "remaining_seconds": 5400 - (cycle as u64 * 30),
                    "code_store": {
                        "q-001": { "python": code_content }
                    },
                    "state_version": cycle as u64
                });

                let sync_res = (*app_clone)
                    .clone()
                    .oneshot(
                        Request::builder()
                            .method("POST")
                            .uri("/api/v1/state/sync")
                            .header(header::CONTENT_TYPE, "application/json")
                            .body(Body::from(serde_json::to_vec(&sync_body).unwrap()))
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(sync_res.status(), StatusCode::OK);
            }
        });
        handles.push(handle);
    }

    // Wait for all 30 concurrent candidate tasks to finish
    for h in handles {
        h.await.unwrap();
    }

    // Verify all 30 files are valid, uncorrupted, complete JSON files on disk
    let candidates_dir = test_dir.join("candidates");
    let entries: Vec<_> = std::fs::read_dir(&candidates_dir).unwrap().collect();
    assert_eq!(entries.len(), NUM_STUDENTS, "Expected exactly {} candidate JSON files on disk", NUM_STUDENTS);

    for i in 1..=NUM_STUDENTS {
        let expected_file = candidates_dir.join(format!("student_{:03}_at_assessment.citadel.org.json", i));
        assert!(expected_file.exists(), "File must exist: {:?}", expected_file);
        let raw = std::fs::read_to_string(&expected_file).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed["candidate_id"], format!("student_{:03}@assessment.citadel.org", i));
        assert_eq!(parsed["state_version"], 5);
        assert!(parsed["code_store"]["q-001"]["python"].as_str().unwrap().contains(&format!("Student {} Cycle 5", i)));
    }

    let _ = std::fs::remove_dir_all(&test_dir);
}

#[tokio::test]
async fn test_heavy_sudden_crash_and_recovery_verification() {
    let test_dir = std::env::temp_dir().join(format!("citadel_heavy_crash_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)));

    const BATCH_SIZE: usize = 10;

    // STEP 1: Boot Server 1, populate roster, have 10 students work
    {
        let app1 = build_app_with_state(AppState::new_with_dir(test_dir.clone()));

        // Upload roster
        let mut roster_cands = Vec::new();
        for i in 1..=BATCH_SIZE {
            roster_cands.push(json!({
                "email": format!("crash_student_{:02}@univ.edu", i),
                "name": format!("Candidate {}", i),
                "section": "Alpha",
                "allowed": true
            }));
        }
        let _ = app1
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/admin/roster/upload?key=citadel-recruiter-key-2026")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(json!({ "exam_id": "CRASH-TEST", "candidates": roster_cands }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // 10 students login and sync their code
        for i in 1..=BATCH_SIZE {
            let email = format!("crash_student_{:02}@univ.edu", i);
            let name = format!("Candidate {}", i);

            let _ = app1
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/v1/auth/login")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(json!({
                            "email": email,
                            "name": name,
                            "passcode": "CITADEL2026"
                        }).to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();

            let code = format!("// Persistent student code for {}\ndef solution(): return {}", email, i * 777);
            let _ = app1
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/v1/state/sync")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(json!({
                            "candidate_id": email,
                            "active_question_id": "q-001",
                            "active_language": "python",
                            "remaining_seconds": 3600 - (i as u64 * 50),
                            "code_store": {
                                "q-001": { "python": code }
                            },
                            "state_version": 10
                        }).to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
        }

        // Simulating immediate KILL -9 of server instance 1
        drop(app1);
    }

    // STEP 2: Boot Server 2 from the exact same state directory
    {
        let app2 = build_app_with_state(AppState::new_with_dir(test_dir.clone()));

        // Check metrics: Recruiter console must immediately show 10 candidates recovered
        let res = app2
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/admin/metrics?key=citadel-recruiter-key-2026")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let dash: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(dash["total_candidates"], BATCH_SIZE);

        // Every student reconnects to Server 2 -> Must receive full resume_state
        for i in 1..=BATCH_SIZE {
            let email = format!("crash_student_{:02}@univ.edu", i);

            let res = app2
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/api/v1/auth/login")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(json!({
                            "email": email,
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

            let expected_code = format!("// Persistent student code for {}\ndef solution(): return {}", email, i * 777);
            assert_eq!(resume.code_store.get("q-001").unwrap().get("python").unwrap(), &expected_code);
            assert_eq!(resume.remaining_seconds, 3600 - (i as u64 * 50));
        }
    }

    let _ = std::fs::remove_dir_all(&test_dir);
}
