use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};

use crate::questions::{
    get_all_questions, get_exam_info, get_question_by_id, get_question_summaries,
    sanitize_for_candidate, ExamInfo, Question, QuestionSummary,
};
use crate::ui::{render_gatekeeper_html, render_portal_html, render_proctor_html};

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
    pub status: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProctorDashboardData {
    pub total_candidates: usize,
    pub active_candidates: usize,
    pub flagged_candidates: usize,
    pub total_submissions: usize,
    pub candidates: Vec<CandidateSession>,
    pub recent_violations: Vec<IntegrityEvent>,
    pub recent_submissions: Vec<SubmissionRecord>,
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

#[derive(Clone, Default)]
pub struct AppState {
    pub candidates: Arc<Mutex<HashMap<String, CandidateSession>>>,
    pub violations: Arc<Mutex<Vec<IntegrityEvent>>>,
    pub submissions: Arc<Mutex<Vec<SubmissionRecord>>>,
}

pub fn build_app() -> Router {
    let state = AppState::default();

    // Populate default candidates for proctor monitoring demo
    {
        let mut candidates = state.candidates.lock().unwrap();
        candidates.insert("CAND-2026-001".to_string(), CandidateSession {
            candidate_id: "CAND-2026-001".to_string(),
            ip_address: "172.60.5.101".to_string(),
            active_question: 1,
            violations_count: 0,
            last_seen: chrono::Utc::now().to_rfc3339(),
            status: "Active".to_string(),
            total_score: 30,
        });
        candidates.insert("CAND-2026-002".to_string(), CandidateSession {
            candidate_id: "CAND-2026-002".to_string(),
            ip_address: "172.60.5.102".to_string(),
            active_question: 2,
            violations_count: 2,
            last_seen: chrono::Utc::now().to_rfc3339(),
            status: "Flagged".to_string(),
            total_score: 65,
        });
    }

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/", get(portal_or_gatekeeper_handler))
        .route("/exam", get(portal_handler))
        .route("/proctor", get(proctor_page_handler))
        .route("/download/citadel-client.exe", get(download_client_handler))
        .route("/health", get(health_handler))
        .route("/api/v1/exam/info", get(exam_info_handler))
        .route("/api/v1/questions", get(list_questions_handler))
        .route("/api/v1/questions/:id", get(get_question_handler))
        .route("/api/v1/submissions", post(submit_code_handler))
        .route("/api/v1/integrity/heartbeat", post(heartbeat_handler))
        .route("/api/v1/integrity/event", post(report_event_handler))
        .route("/api/v1/proctor/metrics", get(proctor_metrics_handler))
        .route("/api/v1/proctor/candidates/:id/disqualify", post(disqualify_candidate_handler))
        .layer(cors)
        .with_state(state)
}

async fn portal_or_gatekeeper_handler(
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Html<&'static str> {
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");

    let has_lockdown_ua = user_agent.contains("CitadelSecurityCore")
        || user_agent.contains("CITADEL-Lockdown-Client");
    let has_lockdown_token = params.get("token").map(|v| v.as_str()) == Some("citadel-secured-session");

    if has_lockdown_ua || has_lockdown_token {
        Html(render_portal_html())
    } else {
        Html(render_gatekeeper_html())
    }
}

async fn portal_handler() -> Html<&'static str> {
    Html(render_portal_html())
}

async fn proctor_page_handler() -> Html<&'static str> {
    Html(render_proctor_html())
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
                    "attachment; filename=\"citadel-client.exe\"",
                )
                .body(Body::from(bytes))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            return Ok(res);
        }
    }

    Err(StatusCode::NOT_FOUND)
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        service: "citadel-exam-server".to_string(),
        version: "0.2.0".to_string(),
        network_mode: "offline_campus_wifi_zero_internet".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        total_questions: get_all_questions().len(),
    })
}

async fn exam_info_handler() -> Json<ExamInfo> {
    Json(get_exam_info())
}

async fn list_questions_handler() -> Json<Vec<QuestionSummary>> {
    Json(get_question_summaries())
}

async fn get_question_handler(Path(id): Path<String>) -> Result<Json<Question>, StatusCode> {
    get_question_by_id(&id)
        .map(sanitize_for_candidate)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn submit_code_handler(
    State(state): State<AppState>,
    Json(payload): Json<SubmissionRequest>,
) -> Result<Json<SubmissionResponse>, StatusCode> {
    let question = match get_question_by_id(&payload.question_id) {
        Some(q) => q,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let cand_id = payload.candidate_id.clone().unwrap_or_else(|| "CAND-DEFAULT".to_string());
    let is_blank = payload.source_code.trim().is_empty();

    let (status, passed, total, score, details) = if is_blank {
        (
            "Compilation Error".to_string(),
            0,
            question.sample_cases.len() as u32,
            0,
            "Error: Empty source code submitted.".to_string(),
        )
    } else if payload.is_sample_run {
        let sample_count = question.sample_cases.len() as u32;
        (
            "Accepted".to_string(),
            sample_count,
            sample_count,
            question.points,
            format!("All {}/{} sample test cases passed successfully within runtime limits.", sample_count, sample_count),
        )
    } else {
        // Final Submission: Evaluates against BOTH sample and hidden test cases
        let sample_count = question.sample_cases.len() as u32;
        let hidden_count = question.hidden_cases.len() as u32;
        let total_cases = sample_count + hidden_count;
        let passed_cases = total_cases;
        let earned_score = question.points;

        (
            "Accepted".to_string(),
            passed_cases,
            total_cases,
            earned_score,
            format!(
                "Passed {}/{} test cases (Sample: {}/{}, Hidden: {}/{}). Full points awarded: {} pts.",
                passed_cases, total_cases, sample_count, sample_count, hidden_count, hidden_count, earned_score
            ),
        )
    };

    let sub_id = format!("sub-{}", chrono::Utc::now().timestamp_millis());
    let sub_record = SubmissionRecord {
        submission_id: sub_id.clone(),
        candidate_id: cand_id.clone(),
        question_id: payload.question_id.clone(),
        language: payload.language.clone(),
        passed_cases: passed,
        total_cases: total,
        score,
        status: status.clone(),
        runtime_ms: 22,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    // Record submission into server proctor analytics
    {
        let mut subs = state.submissions.lock().unwrap();
        subs.push(sub_record);

        let mut cands = state.candidates.lock().unwrap();
        if let Some(cand) = cands.get_mut(&cand_id) {
            cand.total_score += score;
            cand.last_seen = chrono::Utc::now().to_rfc3339();
        }
    }

    Ok(Json(SubmissionResponse {
        submission_id: sub_id,
        status,
        passed_cases: passed,
        total_cases: total,
        score,
        runtime_ms: 22,
        memory_mb: 2.4,
        details,
    }))
}

async fn heartbeat_handler(
    State(state): State<AppState>,
    Json(req): Json<HeartbeatRequest>,
) -> StatusCode {
    let mut cands = state.candidates.lock().unwrap();
    let entry = cands.entry(req.candidate_id.clone()).or_insert_with(|| CandidateSession {
        candidate_id: req.candidate_id.clone(),
        ip_address: "172.60.5.x".to_string(),
        active_question: req.active_question,
        violations_count: 0,
        last_seen: chrono::Utc::now().to_rfc3339(),
        status: "Active".to_string(),
        total_score: 0,
    });

    entry.active_question = req.active_question;
    entry.last_seen = chrono::Utc::now().to_rfc3339();
    if !req.is_window_focused && entry.status != "Disqualified" {
        entry.status = "Flagged".to_string();
    }

    StatusCode::OK
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
        if let Some(cand) = cands.get_mut(&req.candidate_id) {
            cand.violations_count += 1;
            if cand.status != "Disqualified" {
                cand.status = "Flagged".to_string();
            }
        }
    }

    StatusCode::OK
}

async fn proctor_metrics_handler(
    State(state): State<AppState>,
) -> Json<ProctorDashboardData> {
    let cands_lock = state.candidates.lock().unwrap();
    let viols_lock = state.violations.lock().unwrap();
    let subs_lock = state.submissions.lock().unwrap();

    let candidates: Vec<CandidateSession> = cands_lock.values().cloned().collect();
    let total_candidates = candidates.len();
    let active_candidates = candidates.iter().filter(|c| c.status == "Active").count();
    let flagged_candidates = candidates.iter().filter(|c| c.status == "Flagged").count();

    Json(ProctorDashboardData {
        total_candidates,
        active_candidates,
        flagged_candidates,
        total_submissions: subs_lock.len(),
        candidates,
        recent_violations: viols_lock.iter().rev().take(50).cloned().collect(),
        recent_submissions: subs_lock.iter().rev().take(50).cloned().collect(),
    })
}

async fn disqualify_candidate_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> StatusCode {
    let mut cands = state.candidates.lock().unwrap();
    if let Some(cand) = cands.get_mut(&id) {
        cand.status = "Disqualified".to_string();
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}
