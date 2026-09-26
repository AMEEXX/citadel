use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};

use crate::judge::{evaluate_submission, TestCaseDiff};
use crate::questions::{
    get_all_questions, get_exam_info, get_question_summaries, sanitize_for_candidate, ExamInfo,
    Question, QuestionSummary, TestCase,
};
use crate::ui::{
    render_admin_denied_html, render_gatekeeper_html, render_portal_html, render_recruiter_lms_html,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
    pub network_mode: String,
    pub timestamp: String,
    pub total_questions: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubmissionRequest {
    pub question_id: String,
    pub language: String,
    pub source_code: String,
    pub is_sample_run: bool,
    #[serde(default)]
    pub candidate_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubmissionResponse {
    pub submission_id: String,
    pub status: String,
    pub passed_cases: u32,
    pub total_cases: u32,
    pub score: u32,
    pub runtime_ms: u64,
    pub memory_mb: f64,
    pub details: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_diffs: Option<Vec<TestCaseDiff>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityEvent {
    pub id: String,
    pub timestamp: String,
    pub candidate_id: String,
    pub event_type: String,
    pub details: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateSession {
    pub candidate_id: String,
    pub ip_address: String,
    pub active_question: u32,
    pub violations_count: u32,
    pub last_seen: String,
    pub status: String, // "Active", "Flagged", "Disqualified", "Logged Out"
    pub total_score: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionRecord {
    pub submission_id: String,
    pub candidate_id: String,
    pub question_id: String,
    pub language: String,
    pub passed_cases: u32,
    pub total_cases: u32,
    pub score: u32,
    pub status: String,
    pub runtime_ms: u64,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExamLiveState {
    pub is_live: bool,
    pub started_at: Option<String>,
    pub duration_minutes: u32,
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExamStatusResponse {
    pub is_live: bool,
    pub started_at: Option<String>,
    pub remaining_seconds: u64,
    pub total_duration_minutes: u32,
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProctorDashboardData {
    pub total_candidates: usize,
    pub active_candidates: usize,
    pub flagged_candidates: usize,
    pub logged_out_candidates: usize,
    pub disqualified_candidates: usize,
    pub total_submissions: usize,
    pub error_submissions_count: usize,
    pub passed_submissions_count: usize,
    pub candidates: Vec<CandidateSession>,
    pub recent_violations: Vec<IntegrityEvent>,
    pub recent_submissions: Vec<SubmissionRecord>,
    pub questions: Vec<Question>,
    pub exam_live: ExamLiveState,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogoutRequest {
    pub candidate_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HeartbeatRequest {
    pub candidate_id: String,
    pub active_question: u32,
    pub is_window_focused: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReportEventRequest {
    pub candidate_id: String,
    pub event_type: String,
    pub details: String,
    pub severity: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateQuestionPayload {
    pub title: String,
    pub difficulty: String,
    pub points: u32,
    pub description: String,
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateQuestionPayload {
    pub title: String,
    pub difficulty: String,
    pub points: u32,
    pub description: String,
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AddTestCasePayload {
    pub input: String,
    pub expected_output: String,
    pub explanation: Option<String>,
}

#[derive(Clone)]
pub struct AppState {
    pub candidates: Arc<Mutex<HashMap<String, CandidateSession>>>,
    pub violations: Arc<Mutex<Vec<IntegrityEvent>>>,
    pub submissions: Arc<Mutex<Vec<SubmissionRecord>>>,
    pub questions: Arc<RwLock<Vec<Question>>>,
    pub exam_live: Arc<RwLock<ExamLiveState>>,
    pub admin_key: String,
}

impl Default for AppState {
    fn default() -> Self {
        let admin_key = std::env::var("CITADEL_ADMIN_KEY")
            .unwrap_or_else(|_| "citadel-recruiter-key-2026".to_string());

        let initial_live = std::env::var("CITADEL_EXAM_AUTO_LIVE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        AppState {
            candidates: Arc::new(Mutex::new(HashMap::new())),
            violations: Arc::new(Mutex::new(Vec::new())),
            submissions: Arc::new(Mutex::new(Vec::new())),
            questions: Arc::new(RwLock::new(get_all_questions())),
            exam_live: Arc::new(RwLock::new(ExamLiveState {
                is_live: initial_live,
                started_at: if initial_live { Some(chrono::Utc::now().to_rfc3339()) } else { None },
                duration_minutes: 90,
                ended_at: None,
            })),
            admin_key,
        }
    }
}

pub fn is_admin_authorized(headers: &HeaderMap, query: &HashMap<String, String>, state: &AppState) -> bool {
    // 1. Check query param: ?key=...
    if let Some(key) = query.get("key") {
        if key == &state.admin_key {
            return true;
        }
    }
    // 2. Check X-Admin-Key header
    if let Some(h) = headers.get("X-Admin-Key").and_then(|v| v.to_str().ok()) {
        if h == state.admin_key {
            return true;
        }
    }
    // 3. Check Cookie: citadel_admin_key=...
    if let Some(cookie) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
        for part in cookie.split(';') {
            let part = part.trim();
            if let Some(val) = part.strip_prefix("citadel_admin_key=") {
                if val == state.admin_key {
                    return true;
                }
            }
        }
    }
    false
}

pub fn build_app() -> Router {
    let state = AppState::default();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Candidate Portal & Gatekeeper Routes
        .route("/", get(portal_or_gatekeeper_handler))
        .route("/exam", get(portal_handler))
        .route("/download/citadel-client.exe", get(download_client_handler))
        .route("/static/ace.bundle.js", get(serve_ace_bundle_handler))
        .route("/health", get(health_handler))
        .route("/api/v1/exam/info", get(exam_info_handler))
        .route("/api/v1/exam/status", get(exam_status_handler))
        .route("/api/v1/questions", get(list_questions_handler))
        .route("/api/v1/questions/:id", get(get_question_handler))
        .route("/api/v1/submissions", post(submit_code_handler))
        .route("/api/v1/integrity/heartbeat", post(heartbeat_handler))
        .route("/api/v1/integrity/event", post(report_event_handler))
        .route("/api/v1/integrity/logout", post(logout_handler))

        // Protected Recruiter & Administrator Routes
        .route("/admin", get(admin_page_handler))
        .route("/proctor", get(proctor_redirect_handler))
        .route("/api/v1/admin/metrics", get(admin_metrics_handler))
        .route("/api/v1/admin/exam/go-live", post(admin_go_live_handler))
        .route("/api/v1/admin/exam/stop-live", post(admin_stop_live_handler))
        .route("/api/v1/admin/candidates/:id/disqualify", post(admin_disqualify_candidate_handler))
        .route("/api/v1/admin/candidates/:id/clear-flag", post(admin_clear_flag_candidate_handler))
        .route("/api/v1/admin/questions", post(admin_create_question_handler))
        .route("/api/v1/admin/questions/:id", post(admin_update_question_handler))
        .route("/api/v1/admin/questions/:id/sample-cases", post(admin_add_sample_case_handler))
        .route("/api/v1/admin/questions/:id/sample-cases/:idx", delete(admin_delete_sample_case_handler))
        .route("/api/v1/admin/questions/:id/hidden-cases", post(admin_add_hidden_case_handler))
        .route("/api/v1/admin/questions/:id/hidden-cases/:idx", delete(admin_delete_hidden_case_handler))

        // Backward compatibility proctor metrics (checks authorization)
        .route("/api/v1/proctor/metrics", get(proctor_metrics_handler))
        .route("/api/v1/proctor/candidates/:id/disqualify", post(admin_disqualify_candidate_handler))
        .layer(cors)
        .with_state(state)
}

// ============================================================================
// CANDIDATE PORTAL HANDLERS
// ============================================================================

async fn portal_or_gatekeeper_handler(
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let has_lockdown_ua = user_agent.contains("CitadelSecurityCore")
        || user_agent.contains("CITADEL-Lockdown-Client");
    let has_lockdown_token = params.get("token").map(|v| v.as_str()) == Some("citadel-secured-session");
    let has_exam_mode = params.get("mode").map(|v| v.as_str()) == Some("exam");

    let html_content = if has_lockdown_ua || has_lockdown_token || has_exam_mode {
        render_portal_html()
    } else {
        render_gatekeeper_html()
    };

    (
        [
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate, max-age=0"),
            (header::PRAGMA, "no-cache"),
            (header::EXPIRES, "0"),
        ],
        Html(html_content),
    )
}

async fn portal_handler() -> impl IntoResponse {
    (
        [
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate, max-age=0"),
            (header::PRAGMA, "no-cache"),
            (header::EXPIRES, "0"),
        ],
        Html(render_portal_html()),
    )
}

async fn health_handler(State(state): State<AppState>) -> Json<HealthResponse> {
    let questions = state.questions.read().unwrap();
    Json(HealthResponse {
        status: "healthy".to_string(),
        service: "citadel-exam-server".to_string(),
        version: "0.2.0".to_string(),
        network_mode: "offline_campus_wifi_zero_internet".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        total_questions: questions.len(),
    })
}

async fn exam_info_handler() -> Json<ExamInfo> {
    Json(get_exam_info())
}

async fn exam_status_handler(State(state): State<AppState>) -> Json<ExamStatusResponse> {
    let mut live_lock = state.exam_live.write().unwrap();
    let total_secs = (live_lock.duration_minutes as i64) * 60;
    let mut remaining_seconds = 0u64;

    if live_lock.is_live {
        if let Some(ref started_str) = live_lock.started_at {
            if let Ok(started_time) = chrono::DateTime::parse_from_rfc3339(started_str) {
                let now = chrono::Utc::now();
                let elapsed = (now - started_time.with_timezone(&chrono::Utc)).num_seconds();
                if elapsed >= total_secs {
                    // Time expired! Automatically conclude exam
                    live_lock.is_live = false;
                    live_lock.ended_at = Some(now.to_rfc3339());
                    remaining_seconds = 0;
                } else {
                    remaining_seconds = (total_secs - elapsed) as u64;
                }
            } else {
                remaining_seconds = total_secs as u64;
            }
        } else {
            remaining_seconds = total_secs as u64;
        }
    }

    Json(ExamStatusResponse {
        is_live: live_lock.is_live,
        started_at: live_lock.started_at.clone(),
        remaining_seconds,
        total_duration_minutes: live_lock.duration_minutes,
        ended_at: live_lock.ended_at.clone(),
    })
}

async fn list_questions_handler(State(state): State<AppState>) -> Json<Vec<QuestionSummary>> {
    let live = state.exam_live.read().unwrap().is_live;
    if !live {
        return Json(vec![]);
    }
    let questions = state.questions.read().unwrap();
    Json(get_question_summaries(&questions))
}

async fn get_question_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Question>, StatusCode> {
    let live = state.exam_live.read().unwrap().is_live;
    if !live {
        return Err(StatusCode::FORBIDDEN);
    }
    let questions = state.questions.read().unwrap();
    questions
        .iter()
        .find(|q| q.id == id)
        .cloned()
        .map(sanitize_for_candidate)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn submit_code_handler(
    State(state): State<AppState>,
    Json(payload): Json<SubmissionRequest>,
) -> Result<Json<SubmissionResponse>, StatusCode> {
    let is_live = state.exam_live.read().unwrap().is_live;
    if !is_live {
        return Ok(Json(SubmissionResponse {
            submission_id: format!("sub-{}", chrono::Utc::now().timestamp_millis()),
            status: "Exam Inactive".to_string(),
            passed_cases: 0,
            total_cases: 0,
            score: 0,
            runtime_ms: 0,
            memory_mb: 0.0,
            details: "Assessment session is currently not active. Submissions are disabled until recruiter goes live.".to_string(),
            sample_diffs: None,
        }));
    }

    let cand_id = payload.candidate_id.clone().unwrap_or_else(|| "CAND-DEFAULT".to_string());

    // 1. Check if candidate is disqualified
    {
        let cands = state.candidates.lock().unwrap();
        if let Some(cand) = cands.get(&cand_id) {
            if cand.status == "Disqualified" {
                return Ok(Json(SubmissionResponse {
                    submission_id: format!("sub-{}", chrono::Utc::now().timestamp_millis()),
                    status: "Disqualified".to_string(),
                    passed_cases: 0,
                    total_cases: 0,
                    score: 0,
                    runtime_ms: 0,
                    memory_mb: 0.0,
                    details: "Your exam session has been disqualified by the proctor due to security violations. Submissions rejected.".to_string(),
                    sample_diffs: None,
                }));
            }
        }
    }

    // 2. Fetch target question
    let questions = state.questions.read().unwrap();
    let question = match questions.iter().find(|q| q.id == payload.question_id) {
        Some(q) => q.clone(),
        None => return Err(StatusCode::NOT_FOUND),
    };
    drop(questions);

    // 3. Select test cases: Sample Run vs Final Submission
    let (eval_cases, max_points) = if payload.is_sample_run {
        (question.sample_cases.clone(), question.points)
    } else {
        let mut all_cases = question.sample_cases.clone();
        all_cases.extend(question.hidden_cases.clone());
        (all_cases, question.points)
    };

    // 4. REAL Subprocess Execution via Judge Sandbox (Python / C++ / Java)
    let judge_res = evaluate_submission(
        &payload.language,
        &payload.source_code,
        &eval_cases,
        payload.is_sample_run,
        max_points,
    );

    let sub_id = format!("sub-{}", chrono::Utc::now().timestamp_millis());
    let sub_record = SubmissionRecord {
        submission_id: sub_id.clone(),
        candidate_id: cand_id.clone(),
        question_id: payload.question_id.clone(),
        language: payload.language.clone(),
        passed_cases: judge_res.passed_cases,
        total_cases: judge_res.total_cases,
        score: judge_res.score,
        status: judge_res.status.clone(),
        runtime_ms: judge_res.runtime_ms,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    // 5. Update Proctor analytics and candidate score
    {
        let mut subs = state.submissions.lock().unwrap();
        subs.push(sub_record);

        let mut cands = state.candidates.lock().unwrap();
        let cand = cands.entry(cand_id.clone()).or_insert_with(|| CandidateSession {
            candidate_id: cand_id.clone(),
            ip_address: "127.0.0.1".to_string(),
            active_question: 1,
            violations_count: 0,
            last_seen: chrono::Utc::now().to_rfc3339(),
            status: "Active".to_string(),
            total_score: 0,
        });

        if !payload.is_sample_run && judge_res.score > cand.total_score {
            cand.total_score = judge_res.score;
        }
        cand.last_seen = chrono::Utc::now().to_rfc3339();
    }

    Ok(Json(SubmissionResponse {
        submission_id: sub_id,
        status: judge_res.status,
        passed_cases: judge_res.passed_cases,
        total_cases: judge_res.total_cases,
        score: judge_res.score,
        runtime_ms: judge_res.runtime_ms,
        memory_mb: judge_res.memory_mb,
        details: judge_res.details,
        sample_diffs: judge_res.sample_diffs,
    }))
}

async fn heartbeat_handler(
    State(state): State<AppState>,
    Json(req): Json<HeartbeatRequest>,
) -> StatusCode {
    let mut cands = state.candidates.lock().unwrap();
    let entry = cands.entry(req.candidate_id.clone()).or_insert_with(|| CandidateSession {
        candidate_id: req.candidate_id.clone(),
        ip_address: "127.0.0.1".to_string(),
        active_question: req.active_question,
        violations_count: 0,
        last_seen: chrono::Utc::now().to_rfc3339(),
        status: "Active".to_string(),
        total_score: 0,
    });

    entry.active_question = req.active_question;
    entry.last_seen = chrono::Utc::now().to_rfc3339();
    if entry.status != "Disqualified" {
        if !req.is_window_focused {
            entry.status = "Flagged".to_string();
        } else if entry.status == "Logged Out" {
            entry.status = "Active".to_string();
        }
    }

    StatusCode::OK
}

async fn logout_handler(
    State(state): State<AppState>,
    Json(req): Json<LogoutRequest>,
) -> StatusCode {
    let mut cands = state.candidates.lock().unwrap();
    if let Some(cand) = cands.get_mut(&req.candidate_id) {
        if cand.status != "Disqualified" {
            cand.status = "Logged Out".to_string();
        }
        cand.last_seen = chrono::Utc::now().to_rfc3339();
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn report_event_handler(
    State(state): State<AppState>,
    Json(req): Json<ReportEventRequest>,
) -> StatusCode {
    let event = IntegrityEvent {
        id: format!("evt-{}", chrono::Utc::now().timestamp_millis()),
        timestamp: chrono::Utc::now().to_rfc3339(),
        candidate_id: req.candidate_id.clone(),
        event_type: req.event_type.clone(),
        details: req.details.clone(),
        severity: req.severity.unwrap_or_else(|| "HIGH".to_string()),
    };

    {
        let mut viols = state.violations.lock().unwrap();
        viols.push(event);

        let mut cands = state.candidates.lock().unwrap();
        let cand = cands.entry(req.candidate_id.clone()).or_insert_with(|| CandidateSession {
            candidate_id: req.candidate_id.clone(),
            ip_address: "127.0.0.1".to_string(),
            active_question: 1,
            violations_count: 0,
            last_seen: chrono::Utc::now().to_rfc3339(),
            status: "Active".to_string(),
            total_score: 0,
        });

        cand.violations_count += 1;
        if cand.status != "Disqualified" {
            cand.status = "Flagged".to_string();
        }
        cand.last_seen = chrono::Utc::now().to_rfc3339();
    }

    StatusCode::OK
}

async fn download_client_handler() -> Result<Response, StatusCode> {
    let candidates = [
        "target/release/citadel-client.exe",
        "target/debug/citadel-client.exe",
        "../target/release/citadel-client.exe",
        "../target/debug/citadel-client.exe",
        r"\\wsl.localhost\Ubuntu\home\amitlinux\DevProjects\citadel-design\target\release\citadel-client.exe",
        r"\\wsl.localhost\Ubuntu\home\amitlinux\DevProjects\citadel-design\target\debug\citadel-client.exe",
        "/home/amitlinux/DevProjects/citadel-design/target/release/citadel-client.exe",
        "/home/amitlinux/DevProjects/citadel-design/target/debug/citadel-client.exe",
    ];

    for path in &candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let res = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/vnd.microsoft.portable-executable")
                .header(
                    header::CONTENT_DISPOSITION,
                    r#"attachment; filename="citadel-client.exe""#,
                )
                .body(Body::from(bytes))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            return Ok(res);
        }
    }

    Err(StatusCode::NOT_FOUND)
}

// ============================================================================
// PROTECTED RECRUITER & LMS ADMIN HANDLERS
// ============================================================================

async fn admin_page_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Response {
    if !is_admin_authorized(&headers, &query, &state) {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(render_admin_denied_html()))
            .unwrap();
    }

    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store, no-cache, must-revalidate, max-age=0");

    if let Some(key) = query.get("key") {
        let cookie_val = format!("citadel_admin_key={}; Path=/; HttpOnly; SameSite=Lax", key);
        builder = builder.header(header::SET_COOKIE, cookie_val);
    }

    builder.body(Body::from(render_recruiter_lms_html())).unwrap()
}

async fn proctor_redirect_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Response {
    if is_admin_authorized(&headers, &query, &state) {
        Redirect::to("/admin").into_response()
    } else {
        Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(render_admin_denied_html()))
            .unwrap()
    }
}

async fn admin_go_live_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Result<Json<ExamLiveState>, StatusCode> {
    if !is_admin_authorized(&headers, &query, &state) {
        return Err(StatusCode::FORBIDDEN);
    }
    let mut live_lock = state.exam_live.write().unwrap();
    live_lock.is_live = true;
    live_lock.started_at = Some(chrono::Utc::now().to_rfc3339());
    live_lock.ended_at = None;
    Ok(Json(live_lock.clone()))
}

async fn admin_stop_live_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Result<Json<ExamLiveState>, StatusCode> {
    if !is_admin_authorized(&headers, &query, &state) {
        return Err(StatusCode::FORBIDDEN);
    }
    let mut live_lock = state.exam_live.write().unwrap();
    live_lock.is_live = false;
    live_lock.ended_at = Some(chrono::Utc::now().to_rfc3339());
    Ok(Json(live_lock.clone()))
}

async fn admin_metrics_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Result<Json<ProctorDashboardData>, StatusCode> {
    if !is_admin_authorized(&headers, &query, &state) {
        return Err(StatusCode::FORBIDDEN);
    }

    let mut cands_lock = state.candidates.lock().unwrap();
    let now = chrono::Utc::now();

    // Inactivity timeout: candidates not seen for > 30s transition to Logged Out (unless Disqualified)
    for cand in cands_lock.values_mut() {
        if cand.status != "Disqualified" && cand.status != "Logged Out" {
            if let Ok(last) = chrono::DateTime::parse_from_rfc3339(&cand.last_seen) {
                let diff_secs = (now - last.with_timezone(&chrono::Utc)).num_seconds();
                if diff_secs > 30 {
                    cand.status = "Logged Out".to_string();
                }
            }
        }
    }

    let viols_lock = state.violations.lock().unwrap();
    let subs_lock = state.submissions.lock().unwrap();
    let questions_lock = state.questions.read().unwrap();
    let exam_live_lock = state.exam_live.read().unwrap();

    let candidates: Vec<CandidateSession> = cands_lock.values().cloned().collect();
    let total_candidates = candidates.len();
    let active_candidates = candidates.iter().filter(|c| c.status == "Active").count();
    let flagged_candidates = candidates.iter().filter(|c| c.status == "Flagged").count();
    let logged_out_candidates = candidates.iter().filter(|c| c.status == "Logged Out").count();
    let disqualified_candidates = candidates.iter().filter(|c| c.status == "Disqualified").count();

    let error_submissions_count = subs_lock
        .iter()
        .filter(|s| s.status != "Accepted")
        .count();
    let passed_submissions_count = subs_lock
        .iter()
        .filter(|s| s.status == "Accepted")
        .count();

    Ok(Json(ProctorDashboardData {
        total_candidates,
        active_candidates,
        flagged_candidates,
        logged_out_candidates,
        disqualified_candidates,
        total_submissions: subs_lock.len(),
        error_submissions_count,
        passed_submissions_count,
        candidates,
        recent_violations: viols_lock.iter().rev().take(50).cloned().collect(),
        recent_submissions: subs_lock.iter().rev().take(50).cloned().collect(),
        questions: questions_lock.clone(),
        exam_live: exam_live_lock.clone(),
    }))
}

async fn proctor_metrics_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Result<Json<ProctorDashboardData>, StatusCode> {
    if !is_admin_authorized(&headers, &query, &state) {
        return Err(StatusCode::FORBIDDEN);
    }
    admin_metrics_handler(headers, Query(query), State(state)).await
}

async fn admin_disqualify_candidate_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> StatusCode {
    if !is_admin_authorized(&headers, &query, &state) {
        return StatusCode::FORBIDDEN;
    }

    let mut cands = state.candidates.lock().unwrap();
    if let Some(cand) = cands.get_mut(&id) {
        cand.status = "Disqualified".to_string();
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn admin_clear_flag_candidate_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> StatusCode {
    if !is_admin_authorized(&headers, &query, &state) {
        return StatusCode::FORBIDDEN;
    }

    let mut cands = state.candidates.lock().unwrap();
    if let Some(cand) = cands.get_mut(&id) {
        cand.status = "Active".to_string();
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn admin_create_question_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
    Json(payload): Json<CreateQuestionPayload>,
) -> Result<Json<Question>, StatusCode> {
    if !is_admin_authorized(&headers, &query, &state) {
        return Err(StatusCode::FORBIDDEN);
    }

    let mut questions = state.questions.write().unwrap();
    let num = questions.len() + 1;
    let id = format!("q{}-{}", num, payload.title.to_lowercase().replace(' ', "-"));
    let mut starter_templates = HashMap::new();
    starter_templates.insert("python".to_string(), "def solution():\n    pass\n".to_string());
    starter_templates.insert("cpp".to_string(), "#include <iostream>\nusing namespace std;\nint main() {\n    return 0;\n}\n".to_string());
    starter_templates.insert("java".to_string(), "public class Solution {\n    public static void main(String[] args) {}\n}\n".to_string());

    let q = Question {
        id: id.clone(),
        number: num as u32,
        title: payload.title,
        difficulty: payload.difficulty,
        points: payload.points,
        tags: vec!["Algorithms".to_string()],
        description: payload.description,
        input_format: "Standard input format".to_string(),
        output_format: "Standard output format".to_string(),
        constraints: payload.constraints,
        sample_cases: Vec::new(),
        hidden_cases: Vec::new(),
        starter_templates,
    };
    questions.push(q.clone());
    Ok(Json(q))
}

async fn admin_update_question_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateQuestionPayload>,
) -> StatusCode {
    if !is_admin_authorized(&headers, &query, &state) {
        return StatusCode::FORBIDDEN;
    }

    let mut questions = state.questions.write().unwrap();
    if let Some(q) = questions.iter_mut().find(|q| q.id == id) {
        q.title = payload.title;
        q.difficulty = payload.difficulty;
        q.points = payload.points;
        q.description = payload.description;
        q.constraints = payload.constraints;
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn admin_add_sample_case_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<AddTestCasePayload>,
) -> StatusCode {
    if !is_admin_authorized(&headers, &query, &state) {
        return StatusCode::FORBIDDEN;
    }

    let mut questions = state.questions.write().unwrap();
    if let Some(q) = questions.iter_mut().find(|q| q.id == id) {
        q.sample_cases.push(TestCase {
            input: payload.input,
            expected_output: payload.expected_output,
            explanation: payload.explanation,
        });
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn admin_delete_sample_case_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
    Path((id, idx)): Path<(String, usize)>,
) -> StatusCode {
    if !is_admin_authorized(&headers, &query, &state) {
        return StatusCode::FORBIDDEN;
    }

    let mut questions = state.questions.write().unwrap();
    if let Some(q) = questions.iter_mut().find(|q| q.id == id) {
        if idx < q.sample_cases.len() {
            q.sample_cases.remove(idx);
            StatusCode::OK
        } else {
            StatusCode::BAD_REQUEST
        }
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn admin_add_hidden_case_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<AddTestCasePayload>,
) -> StatusCode {
    if !is_admin_authorized(&headers, &query, &state) {
        return StatusCode::FORBIDDEN;
    }

    let mut questions = state.questions.write().unwrap();
    if let Some(q) = questions.iter_mut().find(|q| q.id == id) {
        q.hidden_cases.push(TestCase {
            input: payload.input,
            expected_output: payload.expected_output,
            explanation: payload.explanation,
        });
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn admin_delete_hidden_case_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
    Path((id, idx)): Path<(String, usize)>,
) -> StatusCode {
    if !is_admin_authorized(&headers, &query, &state) {
        return StatusCode::FORBIDDEN;
    }

    let mut questions = state.questions.write().unwrap();
    if let Some(q) = questions.iter_mut().find(|q| q.id == id) {
        if idx < q.hidden_cases.len() {
            q.hidden_cases.remove(idx);
            StatusCode::OK
        } else {
            StatusCode::BAD_REQUEST
        }
    } else {
        StatusCode::NOT_FOUND
    }
}

async fn serve_ace_bundle_handler() -> impl axum::response::IntoResponse {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        include_str!("../static/ace.bundle.js"),
    )
}
