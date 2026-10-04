use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

use citadel_server::{
    api::{CandidateLoginResponse, ProctorDashboardData, SubmissionResponse},
    build_app,
    ui::render_portal_html,
};

async fn set_exam_live(app: &Router) {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/exam/go-live?key=citadel-recruiter-key-2026")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

async fn enroll_candidate(app: &Router, cand_id: &str) {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/roster/add?key=citadel-recruiter-key-2026")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "email": cand_id,
                    "name": format!("Candidate {}", cand_id),
                    "allowed": true
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

const CORRECT_PYTHON_TWO_SUM: &str = r#"import sys

def two_sum(nums, target):
    seen = {}
    for i, n in enumerate(nums):
        diff = target - n
        if diff in seen:
            return [seen[diff], i]
        seen[n] = i
    return []

if __name__ == "__main__":
    lines = sys.stdin.read().split()
    if not lines:
        sys.exit(0)
    target = int(lines[-1])
    nums = [int(x) for x in lines[:-1]]
    res = two_sum(nums, target)
    print(f"{res[0]} {res[1]}")
"#;

#[tokio::test]
async fn test_idempotent_scoring_and_session_sync() {
    let app = build_app();
    set_exam_live(&app).await;

    let cand_id = "cand-idempotent-score-test";
    enroll_candidate(&app, cand_id).await;

    // 1. Initial submission of correct solution (scores 100 points)
    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/submissions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "candidate_id": cand_id,
                    "question_id": "q1-two-sum",
                    "language": "python",
                    "source_code": CORRECT_PYTHON_TWO_SUM,
                    "is_sample_run": false
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res1.status(), StatusCode::OK);
    let body1 = res1.into_body().collect().await.unwrap().to_bytes();
    let sub1: SubmissionResponse = serde_json::from_slice(&body1).unwrap();
    assert_eq!(sub1.status, "Accepted");
    assert_eq!(sub1.score, 100);

    // 2. Second submission of the same solution
    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/submissions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "candidate_id": cand_id,
                    "question_id": "q1-two-sum",
                    "language": "python",
                    "source_code": CORRECT_PYTHON_TWO_SUM,
                    "is_sample_run": false
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::OK);
    let body2 = res2.into_body().collect().await.unwrap().to_bytes();
    let sub2: SubmissionResponse = serde_json::from_slice(&body2).unwrap();
    assert_eq!(sub2.status, "Accepted");
    assert_eq!(sub2.score, 100);

    // 3. Verify in metrics that the candidate's total_score is exactly 100 (NOT 200!)
    let metrics_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/admin/metrics?key=citadel-recruiter-key-2026")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(metrics_res.status(), StatusCode::OK);
    let m_body = metrics_res.into_body().collect().await.unwrap().to_bytes();
    let data: ProctorDashboardData = serde_json::from_slice(&m_body).unwrap();

    let cand_sess = data.candidates.iter().find(|c| c.candidate_id == cand_id).expect("Candidate must exist in metrics");
    assert_eq!(cand_sess.total_score, 100, "Candidate total score must be idempotent (100, not double-counted to 200)");
}

#[tokio::test]
async fn test_session_lifecycle_logout_blocks_reentry() {
    let app = build_app();
    set_exam_live(&app).await;

    let cand_id = "cand-lifecycle-reentry-test";
    enroll_candidate(&app, cand_id).await;

    // 1. Candidate logs in
    let login_res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "email": cand_id,
                    "roll_number": cand_id,
                    "passcode": "CITADEL2026",
                    "client_version": "citadel-portal-v2"
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_res1.status(), StatusCode::OK);

    // 2. Candidate logs out (ends exam voluntarily)
    let logout_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/integrity/logout")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "candidate_id": cand_id,
                    "reason": "Candidate clicked End Exam and submitted session"
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logout_res.status(), StatusCode::OK);

    // 3. Candidate attempts to re-enter -> MUST BE BLOCKED with 403 FORBIDDEN and EXAM_ALREADY_ENDED
    let login_res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "email": cand_id,
                    "roll_number": cand_id,
                    "passcode": "CITADEL2026",
                    "client_version": "citadel-portal-v2"
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_res2.status(), StatusCode::FORBIDDEN);
    let l_body = login_res2.into_body().collect().await.unwrap().to_bytes();
    let l_val: serde_json::Value = serde_json::from_slice(&l_body).unwrap();
    assert_eq!(l_val["error"], "EXAM_ALREADY_ENDED");
    assert!(l_val["message"].as_str().unwrap().contains("candidate_ended"));

    // 4. Admin re-admits candidate via POST /api/v1/admin/candidates/:id/readmit
    let readmit_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/admin/candidates/{}/readmit?key=citadel-recruiter-key-2026", cand_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(readmit_res.status(), StatusCode::OK);

    // 5. Candidate can now successfully re-enter
    let login_res3 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&json!({
                    "email": cand_id,
                    "roll_number": cand_id,
                    "passcode": "CITADEL2026",
                    "client_version": "citadel-portal-v2"
                })).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_res3.status(), StatusCode::OK);
    let l_body3 = login_res3.into_body().collect().await.unwrap().to_bytes();
    let l_resp: CandidateLoginResponse = serde_json::from_slice(&l_body3).unwrap();
    assert_eq!(l_resp.status, "resumed");
}

#[tokio::test]
async fn test_portal_html_features() {
    let portal_html = render_portal_html();

    // Feature 1: Hidden test case visual feedback
    assert!(portal_html.contains("hidden-case"), "Must contain hidden-case CSS class");
    assert!(portal_html.contains("hidden-case-separator"), "Must contain hidden case separator");
    assert!(portal_html.contains("🔒 Case"), "Must contain lock emoji for hidden cases");
    assert!(portal_html.contains("latestHiddenDiffs"), "Must store hidden diffs");
    assert!(portal_html.contains("renderHiddenCaseDetails"), "Must handle hidden case details cleanly");

    // Feature 2: Submit button re-enablement
    assert!(portal_html.contains("unlockSubmitButton()"), "Must re-enable submit button");

    // Feature 4: Session lifecycle overlay for already ended exam
    assert!(portal_html.contains("EXAM_ALREADY_ENDED"), "Must check for EXAM_ALREADY_ENDED");
    assert!(portal_html.contains("showExamAlreadyEndedOverlay"), "Must have exam ended overlay");

    // Feature 5: Enhanced violation reasons
    assert!(portal_html.contains("TAB_HIDDEN"), "Must detect TAB_HIDDEN");
    assert!(portal_html.contains("WINDOW_BLUR_VISIBLE"), "Must detect WINDOW_BLUR_VISIBLE");
    assert!(portal_html.contains("ALT_TAB_DETECTED"), "Must detect ALT_TAB_DETECTED");
    assert!(portal_html.contains("CTRL_TAB_DETECTED"), "Must detect CTRL_TAB_DETECTED");
    assert!(portal_html.contains("DEVTOOLS_SHORTCUT"), "Must detect DEVTOOLS_SHORTCUT");
    assert!(portal_html.contains("SPLIT_SCREEN_DETECTED"), "Must detect SPLIT_SCREEN_DETECTED");
    assert!(portal_html.contains("LARGE_PASTE_DETECTED"), "Must detect LARGE_PASTE_DETECTED");

    // Feature 6: Mischief suppression & calm warning window
    assert!(portal_html.contains("mischief-warning-overlay"), "Must have mischief-warning-overlay element");
    assert!(portal_html.contains("We saw that, don't try it again kid."), "Must display exact calm warning text");
    assert!(portal_html.contains("showMischiefWarning"), "Must have showMischiefWarning handler");
    assert!(portal_html.contains("SCREENSHOT_ATTEMPT"), "Must intercept screenshot shortcuts");

    // Feature 7: Code Autocomplete & IntelliSense
    assert!(portal_html.contains("citadel-ac-popup"), "Must include autocomplete popup element");
    assert!(portal_html.contains("AC_DICTIONARY"), "Must include autocomplete dictionary");
    assert!(portal_html.contains("unordered_map"), "Must include unordered_map completion");
    assert!(portal_html.contains("checkAutocomplete"), "Must include checkAutocomplete function");
    assert!(portal_html.contains("applyAutocomplete"), "Must include applyAutocomplete function");
}
