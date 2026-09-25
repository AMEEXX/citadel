use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

use citadel_server::api::{HealthResponse, SubmissionResponse};
use citadel_server::build_app;
use citadel_server::questions::{ExamInfo, Question, QuestionSummary};

#[tokio::test]
async fn test_health_endpoint() {
    let app = build_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let health: HealthResponse = serde_json::from_slice(&body).unwrap();

    assert_eq!(health.status, "healthy");
    assert_eq!(health.service, "citadel-exam-server");
    assert_eq!(health.network_mode, "offline_campus_wifi_zero_internet");
    assert_eq!(health.total_questions, 3);
}

#[tokio::test]
async fn test_exam_info_endpoint() {
    let app = build_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/exam/info")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let info: ExamInfo = serde_json::from_slice(&body).unwrap();

    assert_eq!(info.duration_minutes, 90);
    assert_eq!(info.total_questions, 3);
    assert_eq!(info.total_points, 100);
}

#[tokio::test]
async fn test_list_questions_endpoint() {
    let app = build_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/questions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let questions: Vec<QuestionSummary> = serde_json::from_slice(&body).unwrap();

    assert_eq!(questions.len(), 3);
    assert_eq!(questions[0].id, "q1-token-bucket");
    assert_eq!(questions[1].id, "q2-subnet-allocator");
    assert_eq!(questions[2].id, "q3-keystroke-outliers");
}

#[tokio::test]
async fn test_get_single_question() {
    let app = build_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/questions/q1-token-bucket")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let q: Question = serde_json::from_slice(&body).unwrap();

    assert_eq!(q.id, "q1-token-bucket");
    assert_eq!(q.points, 30);
    assert!(q.sample_cases.len() >= 2);
    assert!(q.starter_templates.contains_key("python"));
    assert!(q.starter_templates.contains_key("cpp"));
    assert!(q.starter_templates.contains_key("java"));
}

#[tokio::test]
async fn test_get_invalid_question_returns_404() {
    let app = build_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/questions/nonexistent-question-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_code_submission() {
    let app = build_app();
    let payload = serde_json::json!({
        "question_id": "q1-token-bucket",
        "language": "python",
        "source_code": "def solve(): pass",
        "is_sample_run": true
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/submissions")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let sub: SubmissionResponse = serde_json::from_slice(&body).unwrap();

    assert_eq!(sub.status, "Accepted");
    assert_eq!(sub.score, 30);
    assert!(sub.passed_cases >= 2);
}

#[tokio::test]
async fn test_portal_html_serves_offline_app() {
    let app = build_app();
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8(body.to_vec()).unwrap();

    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("CITADEL"));
    assert!(html.contains("CAMPUS PLACEMENT ASSESSMENT"));
    assert!(html.contains("code-editor"));
    assert!(html.contains("results-console"));
}
