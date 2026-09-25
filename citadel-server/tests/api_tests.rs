use axum::{
    body::Body,
    http::{header, Request, StatusCode},
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


#[tokio::test]
async fn test_metrics_alignment_and_candidate_lifecycle() {
    let app = build_app();

    // 1. Initial metrics alignment
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/metrics?key=citadel-recruiter-key-2026")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let data: ProctorDashboardData = serde_json::from_slice(&body).unwrap();

    // Exact mathematical alignment check
    assert_eq!(
        data.total_candidates,
        data.active_candidates + data.flagged_candidates + data.logged_out_candidates + data.disqualified_candidates,
        "Total candidates must exactly equal the sum of all states"
    );

    // 2. Candidate registers via heartbeat
    let new_cand_id = "cand-lifecycle-test-01";
    let hb_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/integrity/heartbeat")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "candidate_id": new_cand_id,
                    "active_question": 1,
                    "is_window_focused": true
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(hb_res.status(), StatusCode::OK);

    // 3. Verify metrics updated
    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/metrics?key=citadel-recruiter-key-2026")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body2 = res2.into_body().collect().await.unwrap().to_bytes();
    let data2: ProctorDashboardData = serde_json::from_slice(&body2).unwrap();

    assert_eq!(data2.total_candidates, data.total_candidates + 1);
    assert_eq!(data2.active_candidates, data.active_candidates + 1);
    assert_eq!(
        data2.total_candidates,
        data2.active_candidates + data2.flagged_candidates + data2.logged_out_candidates + data2.disqualified_candidates
    );
}

#[tokio::test]
async fn test_candidate_explicit_logout_metrics() {
    let app = build_app();
    let cand_id = "cand-logout-test-02";

    // 1. Candidate sends active heartbeat
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/integrity/heartbeat")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "candidate_id": cand_id,
                    "active_question": 1,
                    "is_window_focused": true
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 2. Candidate explicitly logs out
    let logout_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/integrity/logout")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "candidate_id": cand_id,
                    "reason": "Student finished and exited exam"
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logout_res.status(), StatusCode::OK);

    // 3. Metrics reflect Logged Out status
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/metrics?key=citadel-recruiter-key-2026")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let data: ProctorDashboardData = serde_json::from_slice(&body).unwrap();

    let cand = data.candidates.iter().find(|c| c.candidate_id == cand_id).unwrap();
    assert_eq!(cand.status, "Logged Out");
    assert!(data.logged_out_candidates >= 1);
}

#[tokio::test]
async fn test_candidate_focus_loss_flags_candidate() {
    let app = build_app();
    let cand_id = "cand-focus-test-03";

    // Active first
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/integrity/heartbeat")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "candidate_id": cand_id,
                    "active_question": 1,
                    "is_window_focused": true
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Focus lost -> Flagged
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/integrity/heartbeat")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "candidate_id": cand_id,
                    "active_question": 1,
                    "is_window_focused": false
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Check status
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/metrics?key=citadel-recruiter-key-2026")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let data: ProctorDashboardData = serde_json::from_slice(&body).unwrap();
    let cand = data.candidates.iter().find(|c| c.candidate_id == cand_id).unwrap();
    assert_eq!(cand.status, "Flagged");
}

#[tokio::test]
async fn test_submission_error_and_pass_metrics_alignment() {
    let app = build_app();
    let cand_id = "cand-sub-metrics-04";

    // Initial metrics
    let res_init = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/metrics?key=citadel-recruiter-key-2026")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body_init = res_init.into_body().collect().await.unwrap().to_bytes();
    let data_init: ProctorDashboardData = serde_json::from_slice(&body_init).unwrap();

    // 1. Submit bogus code -> Error
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/submissions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "candidate_id": cand_id,
                    "question_id": "q1-two-sum",
                    "language": "python",
                    "source_code": "this code is broken syntax !!!",
                    "is_sample_run": true
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 2. Submit wrong answer -> Error
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/submissions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "candidate_id": cand_id,
                    "question_id": "q1-two-sum",
                    "language": "python",
                    "source_code": "import sys\nprint('999 999')",
                    "is_sample_run": true
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 3. Submit correct Two Sum -> Accepted
    let correct_code = "import sys\ndef two_sum(nums, target):\n    s = {}\n    for i, x in enumerate(nums):\n        if target - x in s: return [s[target-x], i]\n        s[x] = i\n    return []\nif __name__ == '__main__':\n    tokens = sys.stdin.read().split()\n    if tokens:\n        target = int(tokens[-1])\n        nums = [int(x) for x in tokens[:-1]]\n        ans = two_sum(nums, target)\n        if ans: print(f'{min(ans[0], ans[1])} {max(ans[0], ans[1])}')\n";
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/submissions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "candidate_id": cand_id,
                    "question_id": "q1-two-sum",
                    "language": "python",
                    "source_code": correct_code,
                    "is_sample_run": true
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // 4. Verify metrics
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/metrics?key=citadel-recruiter-key-2026")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let data: ProctorDashboardData = serde_json::from_slice(&body).unwrap();

    assert_eq!(data.total_submissions, data_init.total_submissions + 3);
    assert_eq!(data.error_submissions_count, data_init.error_submissions_count + 2);
    assert_eq!(data.passed_submissions_count, data_init.passed_submissions_count + 1);
    assert_eq!(data.total_submissions, data.error_submissions_count + data.passed_submissions_count);
}

#[tokio::test]
async fn test_admin_disqualification_and_submission_block() {
    let app = build_app();
    let cand_id = "cand-disq-test-05";

    // Register candidate
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/integrity/heartbeat")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "candidate_id": cand_id,
                    "active_question": 1,
                    "is_window_focused": true
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Disqualify candidate
    let disq_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/admin/candidates/{}/disqualify?key=citadel-recruiter-key-2026", cand_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disq_res.status(), StatusCode::OK);

    // Verify candidate is Disqualified
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/metrics?key=citadel-recruiter-key-2026")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let data: ProctorDashboardData = serde_json::from_slice(&body).unwrap();
    let cand = data.candidates.iter().find(|c| c.candidate_id == cand_id).unwrap();
    assert_eq!(cand.status, "Disqualified");
    assert!(data.disqualified_candidates >= 1);

    // Disqualified candidate tries to submit -> REJECTED
    let sub_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/submissions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "candidate_id": cand_id,
                    "question_id": "q1-two-sum",
                    "language": "python",
                    "source_code": "print('hello')",
                    "is_sample_run": false
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let sub_body = sub_res.into_body().collect().await.unwrap().to_bytes();
    let sub_data: SubmissionResponse = serde_json::from_slice(&sub_body).unwrap();
    assert_eq!(sub_data.status, "Disqualified");
    assert_eq!(sub_data.score, 0);
}

#[tokio::test]
async fn test_admin_dynamic_testcase_management() {
    let app = build_app();

    // 1. Add new sample test case
    let add_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/questions/q1-two-sum/sample-cases?key=citadel-recruiter-key-2026")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&serde_json::json!({
                    "input": "10 20 30 50",
                    "expected_output": "1 2",
                    "explanation": "20 + 30 = 50"
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_res.status(), StatusCode::OK);

    // Verify sample cases count is now 4
    let q_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/questions/q1-two-sum")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let q_body = q_res.into_body().collect().await.unwrap().to_bytes();
    let q: Question = serde_json::from_slice(&q_body).unwrap();
    assert_eq!(q.sample_cases.len(), 4);
    assert_eq!(q.sample_cases[3].input, "10 20 30 50");

    // 2. Delete the newly added test case
    let del_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/admin/questions/q1-two-sum/sample-cases/3?key=citadel-recruiter-key-2026")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(del_res.status(), StatusCode::OK);

    // Verify restored to 3
    let q_res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/questions/q1-two-sum")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let q_body2 = q_res2.into_body().collect().await.unwrap().to_bytes();
    let q2: Question = serde_json::from_slice(&q_body2).unwrap();
    assert_eq!(q2.sample_cases.len(), 3);
}

#[tokio::test]
async fn test_portal_and_recruiter_ui_rendering_complete() {
    let portal_html = citadel_server::ui::render_portal_html();
    assert!(portal_html.contains("Two Sum"));
    assert!(portal_html.contains("code-editor"));
    assert!(portal_html.contains("submit-btn"));
    assert!(portal_html.contains("lang-select"));
    assert!(portal_html.contains("btn-run"));
    assert!(portal_html.contains("runSampleCases"));
    assert!(portal_html.contains("case-pill-container"));
    assert!(portal_html.contains("beforeunload"));
    assert!(portal_html.contains("navigator.sendBeacon"));

    let recruiter_html = citadel_server::ui::render_recruiter_lms_html();
    assert!(recruiter_html.contains("stat-total"));
    assert!(recruiter_html.contains("stat-active"));
    assert!(recruiter_html.contains("stat-flagged"));
    assert!(recruiter_html.contains("stat-logged-out"));
    assert!(recruiter_html.contains("stat-disqualified"));
    assert!(recruiter_html.contains("stat-subs"));
    assert!(recruiter_html.contains("status-logged-out"));
    assert!(recruiter_html.contains("fetchMetrics"));
}
