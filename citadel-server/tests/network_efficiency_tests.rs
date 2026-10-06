use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::json;
use std::time::Instant;
use tokio::sync::Mutex;
use tower::ServiceExt;

use citadel_server::{
    api::{AppState, SessionControlResponse, TokenSession},
    build_app_with_state,
    questions::Question,
};

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

#[tokio::test]
async fn test_bundle_equals_per_question_responses() {
    let state = AppState::default();
    state.exam_live.write().unwrap().is_live = true;
    let app = build_app_with_state(state);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/questions/bundle")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let bundle: Vec<Question> = serde_json::from_slice(&bytes).unwrap();
    assert!(!bundle.is_empty(), "Bundle should contain questions");

    for q in &bundle {
        let single_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/questions/{}", q.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(single_res.status(), StatusCode::OK);
        let s_bytes = single_res.into_body().collect().await.unwrap().to_bytes();
        let single_q: Question = serde_json::from_slice(&s_bytes).unwrap();

        assert_eq!(q.id, single_q.id);
        assert_eq!(q.title, single_q.title);
        assert_eq!(q.sample_cases.len(), single_q.sample_cases.len());
        assert_eq!(q.hidden_cases, single_q.hidden_cases);
    }
}

#[tokio::test]
async fn test_bundle_never_contains_hidden_cases() {
    let state = AppState::default();
    state.exam_live.write().unwrap().is_live = true;
    let app = build_app_with_state(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/questions/bundle")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let bundle: Vec<Question> = serde_json::from_slice(&bytes).unwrap();

    for q in bundle {
        assert!(
            q.hidden_cases.is_empty(),
            "Question {} leaked hidden cases in bundle",
            q.id
        );
    }
}

#[tokio::test]
async fn test_bundle_caching_and_not_modified() {
    let state = AppState::default();
    state.exam_live.write().unwrap().is_live = true;
    let app = build_app_with_state(state);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/questions/bundle")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let etag = res.headers().get(header::ETAG).cloned();
    assert!(etag.is_some(), "Bundle should include strong ETag");
    let etag_val = etag.unwrap();

    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/questions/bundle")
                .header(header::IF_NONE_MATCH, etag_val)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res2.status(), StatusCode::NOT_MODIFIED);
}

#[tokio::test]
async fn test_bundle_empty_when_not_live() {
    let state = AppState::default();
    state.exam_live.write().unwrap().is_live = false;
    let app = build_app_with_state(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/questions/bundle")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&bytes[..], b"[]");
}

#[tokio::test]
async fn test_sse_kill_switch_and_endpoint() {
    let _guard = ENV_LOCK.lock().await;
    let state = AppState::default();
    let app = build_app_with_state(state);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/events")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let content_type = res.headers().get(header::CONTENT_TYPE).unwrap().to_str().unwrap();
    assert!(content_type.contains("text/event-stream"));

    std::env::set_var("CITADEL_NET_SSE", "0");
    let res_kill = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/events")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    std::env::remove_var("CITADEL_NET_SSE");
    assert_eq!(res_kill.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_longpoll_returns_immediately_when_exit() {
    let _guard = ENV_LOCK.lock().await;
    let state = AppState::default();
    state.exam_live.write().unwrap().is_live = true;
    let app = build_app_with_state(state.clone());

    {
        let mut cands = state.candidates.lock().unwrap();
        if let Some(cand) = cands.get_mut("TEST001") {
            cand.status = "Disqualified".to_string();
        } else {
            cands.insert(
                "TEST001".to_string(),
                citadel_server::api::CandidateSession {
                    candidate_id: "TEST001".to_string(),
                    name: None,
                    ip_address: "127.0.0.1".to_string(),
                    active_question: 1,
                    violations_count: 0,
                    last_seen: chrono::Utc::now().to_rfc3339(),
                    status: "Disqualified".to_string(),
                    total_score: 0,
                    started_at: None,
                    completed_at: None,
                    last_activity_at: None,
                },
            );
        }
    }

    let start = Instant::now();
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/client/session-control?candidate_id=TEST001&wait=25")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let duration = start.elapsed();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(duration.as_millis() < 200, "Should return immediately on exit condition");

    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let sc: SessionControlResponse = serde_json::from_slice(&bytes).unwrap();
    assert!(sc.should_exit);
    assert_eq!(sc.status, "Disqualified");
}

#[tokio::test]
async fn test_longpoll_kill_switch_immediate() {
    let _guard = ENV_LOCK.lock().await;
    let state = AppState::default();
    state.exam_live.write().unwrap().is_live = true;
    let app = build_app_with_state(state);

    std::env::set_var("CITADEL_NET_LONGPOLL", "0");
    let start = Instant::now();
    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/client/session-control?wait=25")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let duration = start.elapsed();
    std::env::remove_var("CITADEL_NET_LONGPOLL");

    assert_eq!(res.status(), StatusCode::OK);
    assert!(duration.as_millis() < 100, "Kill switch should make it return immediately");
}

#[tokio::test]
async fn test_longpoll_wakes_on_force_restore_r1() {
    let _guard = ENV_LOCK.lock().await;
    let state = AppState::default();
    state.exam_live.write().unwrap().is_live = true;

    let test_token = "tok-test-force-restore-123".to_string();
    {
        let mut tokens = state.authorized_tokens.lock().unwrap();
        tokens.insert(
            test_token.clone(),
            TokenSession {
                token: test_token.clone(),
                client_version: "0.2.0".to_string(),
                machine_guid: Some("GUID-TEST".to_string()),
                created_at: chrono::Utc::now(),
                candidate_id: Some("CAND-FORCE".to_string()),
                force_exit: false,
                force_exit_at: None,
            },
        );
    }

    let app = build_app_with_state(state.clone());
    let token_clone = test_token.clone();
    let app_poll = app.clone();

    let poll_task = tokio::spawn(async move {
        let res = app_poll
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/client/session-control?token={}&wait=5", token_clone))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice::<SessionControlResponse>(&bytes).unwrap()
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let fr_res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/client/force-restore")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({
                    "auth_token": test_token,
                    "reason": "Test force restore click"
                }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(fr_res.status(), StatusCode::OK);

    let start_wait = Instant::now();
    let sc_resp = poll_task.await.unwrap();
    let wake_elapsed = start_wait.elapsed();

    assert!(wake_elapsed.as_millis() < 400, "Should wake up rapidly on force-restore bump");
    assert!(sc_resp.should_exit);
    assert_eq!(sc_resp.status, "Restoring");
}
