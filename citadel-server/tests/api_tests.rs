use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

use citadel_server::{
    api::{HealthResponse, ProctorDashboardData, SubmissionResponse},
    build_app,
    questions::{ExamInfo, Question, QuestionSummary},
};

const CORRECT_PYTHON_TWO_SUM: &str = r#"import sys

def two_sum(nums, target):
    seen = {}
    for i, n in enumerate(nums):
        diff = target - n
        if diff in seen:
            return [seen[diff], i]
        seen[n] = i
    return []

if __name__ == '__main__':
    tokens = sys.stdin.read().split()
    if tokens:
        target = int(tokens[-1])
        nums = [int(x) for x in tokens[:-1]]
        ans = two_sum(nums, target)
        if ans and len(ans) == 2:
            print(f"{min(ans[0], ans[1])} {max(ans[0], ans[1])}")
"#;

const WRONG_PYTHON_TWO_SUM: &str = r#"import sys
print("0 0")
"#;

const BOGUS_PYTHON_CODE: &str = "my name is amit and this is bogus code";

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
    assert!(health.total_questions >= 1);
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

    assert_eq!(info.exam_id, "citadel-campus-2026-drive");
    assert_eq!(info.duration_minutes, 90);
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

    assert_eq!(questions.len(), 1);
    assert_eq!(questions[0].id, "q1-two-sum");
    assert_eq!(questions[0].title, "Two Sum");
    assert_eq!(questions[0].points, 100);
}

#[tokio::test]
async fn test_get_single_question() {
    let app = build_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/questions/q1-two-sum")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let q: Question = serde_json::from_slice(&body).unwrap();

    assert_eq!(q.id, "q1-two-sum");
    assert_eq!(q.title, "Two Sum");
    assert!(q.starter_templates.contains_key("cpp"));
    assert!(q.starter_templates.contains_key("python"));
    assert!(q.starter_templates.contains_key("java"));
    assert_eq!(q.sample_cases.len(), 3);
    // Hidden cases must NOT be exposed to candidate
    assert_eq!(q.hidden_cases.len(), 0);
}

#[tokio::test]
async fn test_get_invalid_question_returns_404() {
    let app = build_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/questions/NON-EXISTENT")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_real_judge_correct_python_solution() {
    let app = build_app();
    let payload = json!({
        "question_id": "q1-two-sum",
        "language": "python",
        "source_code": CORRECT_PYTHON_TWO_SUM,
        "is_sample_run": true,
        "candidate_id": "CAND-TEST-ACC"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/submissions")
                .header("Content-Type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let sub: SubmissionResponse = serde_json::from_slice(&body).unwrap();

    assert_eq!(sub.status, "Accepted");
    assert_eq!(sub.passed_cases, 3);
    assert_eq!(sub.total_cases, 3);
    assert!(sub.sample_diffs.is_some());
}

#[tokio::test]
async fn test_real_judge_bogus_code_fails() {
    let app = build_app();
    let payload = json!({
        "question_id": "q1-two-sum",
        "language": "python",
        "source_code": BOGUS_PYTHON_CODE,
        "is_sample_run": true,
        "candidate_id": "CAND-TEST-BOGUS"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/submissions")
                .header("Content-Type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let sub: SubmissionResponse = serde_json::from_slice(&body).unwrap();

    // Bogus code MUST NOT pass
    assert_ne!(sub.status, "Accepted");
    assert!(sub.status == "Compilation Error" || sub.status == "Runtime Error");
    assert_eq!(sub.passed_cases, 0);
}

#[tokio::test]
async fn test_real_judge_wrong_answer() {
    let app = build_app();
    let payload = json!({
        "question_id": "q1-two-sum",
        "language": "python",
        "source_code": WRONG_PYTHON_TWO_SUM,
        "is_sample_run": true,
        "candidate_id": "CAND-TEST-WA"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/submissions")
                .header("Content-Type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let sub: SubmissionResponse = serde_json::from_slice(&body).unwrap();

    assert_eq!(sub.status, "Wrong Answer");
    assert_eq!(sub.passed_cases, 0);
}

#[tokio::test]
async fn test_final_submission_evaluates_hidden_cases() {
    let app = build_app();
    let payload = json!({
        "question_id": "q1-two-sum",
        "language": "python",
        "source_code": CORRECT_PYTHON_TWO_SUM,
        "is_sample_run": false,
        "candidate_id": "CAND-TEST-FINAL"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/submissions")
                .header("Content-Type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let sub: SubmissionResponse = serde_json::from_slice(&body).unwrap();

    assert_eq!(sub.status, "Accepted");
    assert_eq!(sub.score, 100);
    // 3 sample + 5 hidden = 8 total cases
    assert_eq!(sub.total_cases, 8);
    assert_eq!(sub.passed_cases, 8);
}

#[tokio::test]
async fn test_admin_protection_and_authorization() {
    let app = build_app();

    // 1. Unauthenticated request to /admin -> 403 Forbidden
    let unauth_res = app.clone()
        .oneshot(Request::builder().uri("/admin").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(unauth_res.status(), StatusCode::FORBIDDEN);

    // 2. Unauthenticated request to /api/v1/admin/metrics -> 403 Forbidden
    let unauth_api = app.clone()
        .oneshot(Request::builder().uri("/api/v1/admin/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(unauth_api.status(), StatusCode::FORBIDDEN);

    // 3. Authenticated request with key -> 200 OK
    let auth_res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/metrics?key=citadel-recruiter-key-2026")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(auth_res.status(), StatusCode::OK);
    let body = auth_res.into_body().collect().await.unwrap().to_bytes();
    let data: ProctorDashboardData = serde_json::from_slice(&body).unwrap();
    assert!(data.questions.len() >= 1);
    assert_eq!(data.questions[0].id, "q1-two-sum");
}

#[tokio::test]
async fn test_portal_gatekeeper_serves_download_link() {
    let app = build_app();
    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8(body.to_vec()).unwrap();

    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("CITADEL Assessment Appliance"));
    assert!(html.contains("/download/citadel-client.exe"));
}

#[tokio::test]
async fn test_secured_portal_html_serves_exam() {
    let app = build_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/?token=citadel-secured-session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8(body.to_vec()).unwrap();

    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("CITADEL"));
    assert!(html.contains("Two Sum"));
    assert!(html.contains("code-editor"));
    assert!(html.contains("submit-btn"));
    assert!(html.contains("drawer-console"));
}
