use crate::persistence::{
    load_all_candidate_states, load_roster, parse_roster_csv,
    save_candidate_state, save_roster, save_submission_snapshot, save_violation_snapshot,
    CandidateResumeState, CandidateState, ExamRoster, RosterEntry,
};
use std::path::PathBuf;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
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
    render_admin_denied_html, render_gatekeeper_html, render_mobile_blocked_html, render_portal_html, render_recruiter_lms_html,
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
    #[serde(default)]
    pub name: Option<String>,
    pub ip_address: String,
    pub active_question: u32,
    pub violations_count: u32,
    pub last_seen: String,
    pub status: String, // "Active", "Flagged", "Disqualified", "Logged Out"
    pub total_score: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateSubmissionDetail {
    pub submission_id: String,
    pub candidate_id: String,
    pub question_id: String,
    pub question_title: String,
    pub language: String,
    pub passed_cases: u32,
    pub total_cases: u32,
    pub score: u32,
    pub status: String,
    pub runtime_ms: u64,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateProfileResponse {
    pub candidate_id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub ip_address: String,
    pub status: String,
    pub total_score: u32,
    pub active_question: u32,
    pub violations_count: u32,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub last_seen: String,
    pub total_submissions: usize,
    pub passed_submissions: usize,
    pub total_violations: usize,
    pub submissions: Vec<CandidateSubmissionDetail>,
    pub integrity_events: Vec<IntegrityEvent>,
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
    #[serde(default)]
    pub is_production: bool,
    #[serde(default)]
    pub early_exit_min_remaining_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSession {
    pub token: String,
    pub client_version: String,
    pub machine_guid: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClientHandshakeRequest {
    pub client_version: String,
    pub machine_guid: Option<String>,
    pub mode: Option<String>,
    pub is_elevated: Option<bool>,
    pub elevation_proof: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClientHandshakeResponse {
    pub status: String,
    pub session_token: String,
    pub is_production: bool,
    pub server_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminModeResponse {
    pub is_production: bool,
    pub mode: String,
    pub active_authorized_tokens: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SetModeRequest {
    pub production: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProctorDashboardData {
    pub is_production: bool,
    pub total_candidates: usize,
    pub active_candidates: usize,
    pub flagged_candidates: usize,
    pub logged_out_candidates: usize,
    pub disqualified_candidates: usize,
    #[serde(default)]
    pub disconnected_candidates: usize,
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


#[derive(Debug, Clone, Serialize)]
pub struct HeartbeatResponse {
    pub status: String,
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
    pub candidate_states: Arc<Mutex<HashMap<String, CandidateState>>>,
    pub roster: Arc<RwLock<ExamRoster>>,
    pub exam_passcode: Arc<RwLock<String>>,
    pub state_dir: PathBuf,
    pub violations: Arc<Mutex<Vec<IntegrityEvent>>>,
    pub submissions: Arc<Mutex<Vec<SubmissionRecord>>>,
    pub questions: Arc<RwLock<Vec<Question>>>,
    pub exam_live: Arc<RwLock<ExamLiveState>>,
    pub admin_key: String,
    pub is_production: Arc<AtomicBool>,
    pub authorized_tokens: Arc<Mutex<HashMap<String, TokenSession>>>,
}

impl AppState {
    pub fn new_with_dir(state_dir: PathBuf) -> Self {
        let admin_key = std::env::var("CITADEL_ADMIN_KEY")
            .unwrap_or_else(|_| "citadel-recruiter-key-2026".to_string());

        let initial_live = std::env::var("CITADEL_EXAM_AUTO_LIVE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(true);

        let is_prod_val = std::env::var("CITADEL_PRODUCTION")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let exam_passcode = std::env::var("CITADEL_EXAM_PASSCODE")
            .unwrap_or_else(|_| "CITADEL2026".to_string());

        let _ = crate::persistence::ensure_directories(&state_dir);

        let roster = load_roster(&state_dir).unwrap_or_default();
        let saved_states = load_all_candidate_states(&state_dir).unwrap_or_default();

        let mut candidates_map = HashMap::new();
        let mut tokens_map = HashMap::new();

        for (cid, cstate) in &saved_states {
            candidates_map.insert(
                cid.clone(),
                CandidateSession {
                    candidate_id: cstate.candidate_id.clone(),
                    name: Some(cstate.name.clone()),
                    ip_address: cstate.ip_address.clone(),
                    active_question: cstate.active_question_id.replace("q-", "").parse().unwrap_or(1),
                    violations_count: cstate.violations_count,
                    last_seen: cstate.last_seen.clone(),
                    status: cstate.status.clone(),
                    total_score: cstate.total_score,
                    started_at: cstate.started_at.clone(),
                    completed_at: cstate.completed_at.clone(),
                },
            );

            if !cstate.session_token.is_empty() {
                tokens_map.insert(
                    cstate.session_token.clone(),
                    TokenSession {
                        token: cstate.session_token.clone(),
                        client_version: "restored".to_string(),
                        machine_guid: None,
                        created_at: chrono::Utc::now(),
                    },
                );
            }
        }

        AppState {
            candidates: Arc::new(Mutex::new(candidates_map)),
            candidate_states: Arc::new(Mutex::new(saved_states)),
            roster: Arc::new(RwLock::new(roster)),
            exam_passcode: Arc::new(RwLock::new(exam_passcode)),
            state_dir,
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
            is_production: Arc::new(AtomicBool::new(is_prod_val)),
            authorized_tokens: Arc::new(Mutex::new(tokens_map)),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        let admin_key = std::env::var("CITADEL_ADMIN_KEY")
            .unwrap_or_else(|_| "citadel-recruiter-key-2026".to_string());

        let initial_live = std::env::var("CITADEL_EXAM_AUTO_LIVE")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let is_prod_val = std::env::var("CITADEL_PRODUCTION")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let exam_passcode = std::env::var("CITADEL_EXAM_PASSCODE")
            .unwrap_or_else(|_| "CITADEL2026".to_string());

        let state_dir = std::env::var("CITADEL_STATE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                if PathBuf::from("citadel-server/state").exists() {
                    PathBuf::from("citadel-server/state")
                } else if PathBuf::from("../citadel-server/state").exists() {
                    PathBuf::from("../citadel-server/state")
                } else {
                    PathBuf::from("./state")
                }
            });

        let _ = crate::persistence::ensure_directories(&state_dir);

        let roster = load_roster(&state_dir).unwrap_or_default();
        let saved_states = load_all_candidate_states(&state_dir).unwrap_or_default();

        let mut candidates_map = HashMap::new();
        let mut tokens_map = HashMap::new();

        for (cid, cstate) in &saved_states {
            candidates_map.insert(
                cid.clone(),
                CandidateSession {
                    candidate_id: cstate.candidate_id.clone(),
                    name: Some(cstate.name.clone()),
                    ip_address: cstate.ip_address.clone(),
                    active_question: cstate.active_question_id.replace("q-", "").parse().unwrap_or(1),
                    violations_count: cstate.violations_count,
                    last_seen: cstate.last_seen.clone(),
                    status: cstate.status.clone(),
                    total_score: cstate.total_score,
                    started_at: cstate.started_at.clone(),
                    completed_at: cstate.completed_at.clone(),
                },
            );

            if !cstate.session_token.is_empty() {
                tokens_map.insert(
                    cstate.session_token.clone(),
                    TokenSession {
                        token: cstate.session_token.clone(),
                        client_version: "restored".to_string(),
                        machine_guid: None,
                        created_at: chrono::Utc::now(),
                    },
                );
            }
        }

        eprintln!(
            "[CITADEL SERVER PERSISTENCE] Loaded {} candidates and {} roster entries from disk ({})",
            saved_states.len(),
            roster.candidates.len(),
            state_dir.display()
        );

        AppState {
            candidates: Arc::new(Mutex::new(candidates_map)),
            candidate_states: Arc::new(Mutex::new(saved_states)),
            roster: Arc::new(RwLock::new(roster)),
            exam_passcode: Arc::new(RwLock::new(exam_passcode)),
            state_dir,
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
            is_production: Arc::new(AtomicBool::new(is_prod_val)),
            authorized_tokens: Arc::new(Mutex::new(tokens_map)),
        }
    }
}

pub fn is_request_authorized(
    headers: &HeaderMap,
    query: &HashMap<String, String>,
    state: &AppState,
) -> bool {
    // In Testing Mode (CITADEL_PRODUCTION=0), open network access is permitted for rapid testing
    if !state.is_production.load(Ordering::SeqCst) {
        return true;
    }

    // In Production Mode, strictly require a valid session token from citadel-client handshake
    // 1. Query parameter: ?auth_token=... or ?token=...
    if let Some(token) = query.get("auth_token").or_else(|| query.get("token")) {
        let tokens = state.authorized_tokens.lock().unwrap();
        if tokens.contains_key(token) {
            return true;
        }
    }

    // 2. HTTP header: X-Citadel-Auth-Token
    if let Some(h) = headers.get("X-Citadel-Auth-Token").and_then(|v| v.to_str().ok()) {
        let tokens = state.authorized_tokens.lock().unwrap();
        if tokens.contains_key(h) {
            return true;
        }
    }

    // 3. HTTP Cookie: citadel_auth_token=...
    if let Some(cookie) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
        for part in cookie.split(';') {
            let part = part.trim();
            if let Some(val) = part.strip_prefix("citadel_auth_token=") {
                let tokens = state.authorized_tokens.lock().unwrap();
                if tokens.contains_key(val) {
                    return true;
                }
            }
        }
    }

    false
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
    build_app_with_state(AppState::default())
}

pub fn build_app_with_state(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let router = Router::new()
        // Candidate Portal & Gatekeeper Routes
        .route("/", get(portal_or_gatekeeper_handler))
        .route("/exam", get(portal_handler))
        .route("/download/citadel-client.exe", get(download_client_handler))
        .route("/static/ace.bundle.js", get(serve_ace_bundle_handler))
        .route("/static/*path", get(serve_static_handler))
        .route("/architecture", get(architecture_handler))
        .route("/archify", get(architecture_handler))
        .route("/health", get(health_handler))
        .route("/api/v1/exam/info", get(exam_info_handler))
        .route("/api/v1/exam/status", get(exam_status_handler))
        .route("/api/v1/questions", get(list_questions_handler))
        .route("/api/v1/questions/:id", get(get_question_handler))
        .route("/api/v1/submissions", post(submit_code_handler))
        .route("/api/v1/integrity/heartbeat", post(heartbeat_handler))
        .route("/api/v1/integrity/event", post(report_event_handler))
        .route("/api/v1/integrity/logout", post(logout_handler))
        .route("/api/v1/client/session-control", get(client_session_control_handler))
        .route("/api/v1/client/handshake", post(client_handshake_handler))
        .route("/api/v1/client/end-exam", post(client_end_exam_handler))
        .route("/api/v1/client/kill-all-lockdown", post(kill_all_lockdown_handler))
        // Candidate Roster Authentication & State Persistence Routes
        .route("/api/v1/auth/login", post(candidate_login_handler))
        .route("/api/v1/state/sync", post(state_sync_handler))
        .route("/api/v1/state/restore", get(state_restore_handler))
        .route("/api/v1/admin/roster", get(admin_get_roster_handler))
        .route("/api/v1/admin/roster/upload", post(admin_upload_roster_handler))
        .route("/api/v1/admin/roster/add", post(admin_add_roster_candidate_handler))
        .route("/api/v1/admin/roster/:roll", delete(admin_delete_roster_candidate_handler))
        .route("/api/v1/admin/candidates/:id/extend-time", post(admin_extend_candidate_time_handler))
        .route("/api/v1/admin/exam/passcode", get(admin_get_passcode_handler).post(admin_set_passcode_handler))

        // Protected Recruiter & Administrator Routes
        .route("/admin", get(admin_page_handler))
        .route("/proctor", get(proctor_redirect_handler))
        .route("/api/v1/admin/metrics", get(admin_metrics_handler))
        .route("/api/v1/admin/mode", get(get_admin_mode_handler))
        .route("/api/v1/admin/mode/toggle", post(toggle_admin_mode_handler))
        .route("/api/v1/admin/mode/set", post(set_admin_mode_handler))
        .route("/api/v1/admin/exam/go-live", post(admin_go_live_handler))
        .route("/api/v1/admin/exam/stop-live", post(admin_stop_live_handler))
        .route("/api/v1/admin/candidates/:id/disqualify", post(admin_disqualify_candidate_handler))
        .route("/api/v1/admin/candidates/:id/clear-flag", post(admin_clear_flag_candidate_handler))
        .route("/api/v1/admin/questions", post(admin_create_question_handler))
        .route("/api/v1/admin/candidates/:id/profile", get(admin_candidate_profile_handler))
        .route("/api/v1/admin/questions/:id", post(admin_update_question_handler).delete(admin_delete_question_handler))
        .route("/api/v1/admin/questions/:id/sample-cases", post(admin_add_sample_case_handler))
        .route("/api/v1/admin/questions/:id/sample-cases/:idx", delete(admin_delete_sample_case_handler))
        .route("/api/v1/admin/questions/:id/hidden-cases", post(admin_add_hidden_case_handler))
        .route("/api/v1/admin/questions/:id/hidden-cases/:idx", delete(admin_delete_hidden_case_handler))

        // Backward compatibility proctor metrics (checks authorization)
        .route("/api/v1/proctor/metrics", get(proctor_metrics_handler))
        .route("/api/v1/proctor/candidates/:id/disqualify", post(admin_disqualify_candidate_handler))
        .layer(cors)
        .with_state(state.clone());

    // Spawn disconnect watchdog task
    let watchdog_state = state.clone();
    tokio::spawn(async move {
        disconnect_watchdog_loop(watchdog_state).await;
    });

    router
}

// ============================================================================
// CANDIDATE PORTAL HANDLERS
// ============================================================================

/// Returns true if the provided User-Agent header indicates a mobile phone or tablet device.
pub fn is_mobile_or_tablet_user_agent(user_agent: &str) -> bool {
    let ua = user_agent.to_lowercase();
    // Exclude Citadel client / desktop clients
    if ua.contains("citadelsecuritycore") || ua.contains("citadel-lockdown-client") {
        return false;
    }

    ua.contains("mobile")
        || ua.contains("android")
        || ua.contains("iphone")
        || ua.contains("ipad")
        || ua.contains("ipod")
        || ua.contains("blackberry")
        || ua.contains("iemobile")
        || ua.contains("opera mini")
        || ua.contains("opera mobi")
        || ua.contains("tablet")
        || ua.contains("silk/")
        || ua.contains("fennec")
        || ua.contains("kindle")
}


async fn portal_or_gatekeeper_handler(
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let has_token_param = params.get("auth_token").or_else(|| params.get("token")).is_some();
    let has_token_cookie = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(|c| c.contains("citadel_auth_token="))
        .unwrap_or(false);
    let has_auth_header = headers.get("X-Citadel-Auth-Token").is_some();
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let has_lockdown_ua = user_agent.contains("CitadelSecurityCore")
        || user_agent.contains("CITADEL-Lockdown-Client");
    let is_prod = state.is_production.load(Ordering::SeqCst);
    let is_mobile = is_mobile_or_tablet_user_agent(user_agent);

    let is_presenting_auth = has_token_param || has_token_cookie || has_auth_header || has_lockdown_ua;
    let authorized = is_presenting_auth && is_request_authorized(&headers, &params, &state);

    if authorized {
        if is_prod && is_mobile {
            return (
                StatusCode::FORBIDDEN,
                [
                    (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate, max-age=0"),
                    (header::PRAGMA, "no-cache"),
                    (header::EXPIRES, "0"),
                ],
                Html(render_mobile_blocked_html().to_string()),
            ).into_response();
        }

        let mut response = (
            [
                (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate, max-age=0"),
                (header::PRAGMA, "no-cache"),
                (header::EXPIRES, "0"),
            ],
            Html(render_portal_html()),
        ).into_response();

        if let Some(token) = params.get("auth_token").or_else(|| params.get("token")) {
            let cookie_header = format!("citadel_auth_token={}; Path=/; SameSite=Lax; Max-Age=28800", token);
            if let Ok(val) = cookie_header.parse() {
                response.headers_mut().insert(header::SET_COOKIE, val);
            }
        }
        response
    } else {
        (
            [
                (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate, max-age=0"),
                (header::PRAGMA, "no-cache"),
                (header::EXPIRES, "0"),
            ],
            Html(render_gatekeeper_html(is_prod, is_mobile)),
        ).into_response()
    }
}

async fn portal_handler(
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let is_prod = state.is_production.load(Ordering::SeqCst);
    let is_mobile = is_mobile_or_tablet_user_agent(user_agent);

    if is_prod && is_mobile {
        return (
            StatusCode::FORBIDDEN,
            [
                (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate, max-age=0"),
                (header::PRAGMA, "no-cache"),
                (header::EXPIRES, "0"),
            ],
            Html(render_mobile_blocked_html().to_string()),
        ).into_response();
    }

    let authorized = is_request_authorized(&headers, &params, &state);

    if !authorized {
        return Redirect::to("/").into_response();
    }

    let mut response = (
        [
            (header::CACHE_CONTROL, "no-store, no-cache, must-revalidate, max-age=0"),
            (header::PRAGMA, "no-cache"),
            (header::EXPIRES, "0"),
        ],
        Html(render_portal_html()),
    ).into_response();

    if let Some(token) = params.get("auth_token").or_else(|| params.get("token")) {
        let cookie_header = format!("citadel_auth_token={}; Path=/; SameSite=Lax; Max-Age=28800", token);
        if let Ok(val) = cookie_header.parse() {
            response.headers_mut().insert(header::SET_COOKIE, val);
        }
    }
    response
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
        is_production: state.is_production.load(Ordering::SeqCst),
        early_exit_min_remaining_seconds: 900,
    })
}

async fn list_questions_handler(
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if !is_request_authorized(&headers, &params, &state) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "unauthorized_client",
                "code": "CITADEL_LOCKDOWN_REQUIRED",
                "message": "Access restricted. Direct web access is blocked in Production Mode. Please open the assessment using the official Citadel Lockdown Client."
            })),
        ).into_response();
    }

    let live = state.exam_live.read().unwrap().is_live;
    if !live {
        return Json(Vec::<QuestionSummary>::new()).into_response();
    }
    let questions = state.questions.read().unwrap();
    Json(get_question_summaries(&questions)).into_response()
}

async fn get_question_handler(
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if !is_request_authorized(&headers, &params, &state) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "unauthorized_client",
                "code": "CITADEL_LOCKDOWN_REQUIRED",
                "message": "Access restricted. Direct web access is blocked in Production Mode. Please open the assessment using the official Citadel Lockdown Client."
            })),
        ).into_response();
    }

    let live = state.exam_live.read().unwrap().is_live;
    if !live {
        return StatusCode::FORBIDDEN.into_response();
    }
    let questions = state.questions.read().unwrap();
    match questions.iter().find(|q| q.id == id).cloned().map(sanitize_for_candidate) {
        Some(q) => Json(q).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn submit_code_handler(
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
    Json(payload): Json<SubmissionRequest>,
) -> impl IntoResponse {
    if !is_request_authorized(&headers, &params, &state) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "unauthorized_client",
                "code": "CITADEL_LOCKDOWN_REQUIRED",
                "message": "Submissions are restricted. Submissions must originate from the official Citadel Lockdown Client."
            })),
        ).into_response();
    }
    let is_live = state.exam_live.read().unwrap().is_live;
    if !is_live {
        return Json(SubmissionResponse {
            submission_id: format!("sub-{}", chrono::Utc::now().timestamp_millis()),
            status: "Exam Inactive".to_string(),
            passed_cases: 0,
            total_cases: 0,
            score: 0,
            runtime_ms: 0,
            memory_mb: 0.0,
            details: "Assessment session is currently not active. Submissions are disabled until recruiter goes live.".to_string(),
            sample_diffs: None,
        }).into_response();
    }

    let cand_id = payload.candidate_id.clone().unwrap_or_else(|| "CAND-DEFAULT".to_string()).trim().to_string();

    // Verify candidate against roster
    {
        let roster = state.roster.read().unwrap();
        if !roster.candidates.is_empty() {
            if let Err((status, code, msg)) = check_candidate_roster(&cand_id, &roster) {
                return (
                    status,
                    Json(serde_json::json!({
                        "error": code,
                        "message": msg
                    })),
                ).into_response();
            }
        }
    }

    // 1. Check if candidate is disqualified
    {
        let cands = state.candidates.lock().unwrap();
        if let Some(cand) = cands.get(&cand_id) {
            if cand.status == "Disqualified" {
                return Json(SubmissionResponse {
                    submission_id: format!("sub-{}", chrono::Utc::now().timestamp_millis()),
                    status: "Disqualified".to_string(),
                    passed_cases: 0,
                    total_cases: 0,
                    score: 0,
                    runtime_ms: 0,
                    memory_mb: 0.0,
                    details: "Your exam session has been disqualified by the proctor due to security violations. Submissions rejected.".to_string(),
                    sample_diffs: None,
                }).into_response();
            }
        }
    }

    // 2. Fetch target question
    let questions = state.questions.read().unwrap();
    let question = match questions.iter().find(|q| q.id == payload.question_id) {
        Some(q) => q.clone(),
        None => return StatusCode::NOT_FOUND.into_response(),
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
        subs.push(sub_record.clone());

        let mut cands = state.candidates.lock().unwrap();
        let cand = cands.entry(cand_id.clone()).or_insert_with(|| CandidateSession {
            candidate_id: cand_id.clone(),
            name: None,
            ip_address: "127.0.0.1".to_string(),
            active_question: 1,
            violations_count: 0,
            last_seen: chrono::Utc::now().to_rfc3339(),
            status: "Active".to_string(),
            total_score: 0,
            started_at: Some(chrono::Utc::now().to_rfc3339()),
            completed_at: None,
        });

        if !payload.is_sample_run && judge_res.score > cand.total_score {
            cand.total_score = judge_res.score;
        }
        cand.last_seen = chrono::Utc::now().to_rfc3339();
    }

    // Persist to CandidateState and Submissions
    {
        let mut c_states = state.candidate_states.lock().unwrap();
        let cand_st = c_states.entry(cand_id.clone()).or_insert_with(|| CandidateState {
            candidate_id: cand_id.clone(),
            name: cand_id.clone(),
            ip_address: "127.0.0.1".to_string(),
            active_question_id: payload.question_id.clone(),
            active_language: payload.language.clone(),
            code_store: HashMap::new(),
            remaining_seconds: 90 * 60,
            timer_paused_at: None,
            best_scores: HashMap::new(),
            total_score: 0,
            status: "Active".to_string(),
            violations_count: 0,
            started_at: Some(chrono::Utc::now().to_rfc3339()),
            last_seen: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
            session_token: String::new(),
            state_version: 1,
            last_synced_at: chrono::Utc::now().to_rfc3339(),
        });

        if !payload.is_sample_run {
            let curr_best = cand_st.best_scores.entry(payload.question_id.clone()).or_insert(0);
            if judge_res.score > *curr_best {
                *curr_best = judge_res.score;
            }
            cand_st.total_score = cand_st.best_scores.values().sum();
        }
        cand_st.last_seen = chrono::Utc::now().to_rfc3339();
        let _ = save_candidate_state(&state.state_dir, cand_st);
    }
    let _ = save_submission_snapshot(&state.state_dir, &sub_record.submission_id, &sub_record);

    Json(SubmissionResponse {
        submission_id: sub_id,
        status: judge_res.status,
        passed_cases: judge_res.passed_cases,
        total_cases: judge_res.total_cases,
        score: judge_res.score,
        runtime_ms: judge_res.runtime_ms,
        memory_mb: judge_res.memory_mb,
        details: judge_res.details,
        sample_diffs: judge_res.sample_diffs,
    }).into_response()
}

async fn heartbeat_handler(
    State(state): State<AppState>,
    Json(req): Json<HeartbeatRequest>,
) -> impl IntoResponse {
    let cid = req.candidate_id.trim();
    if cid.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "MISSING_IDENTIFIER",
                "message": "Candidate identifier is required."
            })),
        ).into_response();
    }

    // Verify candidate is enrolled in roster
    {
        let roster = state.roster.read().unwrap();
        if !roster.candidates.is_empty() {
            if let Err((status, code, msg)) = check_candidate_roster(cid, &roster) {
                return (
                    status,
                    Json(serde_json::json!({
                        "error": code,
                        "message": msg,
                        "status": "Unauthorized"
                    })),
                ).into_response();
            }
        }
    }

    let mut cands = state.candidates.lock().unwrap();
    let entry = cands.entry(cid.to_string()).or_insert_with(|| CandidateSession {
        candidate_id: cid.to_string(),
        name: None,
        ip_address: "127.0.0.1".to_string(),
        active_question: req.active_question,
        violations_count: 0,
        last_seen: chrono::Utc::now().to_rfc3339(),
        status: "Active".to_string(),
        total_score: 0,
        started_at: Some(chrono::Utc::now().to_rfc3339()),
        completed_at: None,
    });

    entry.active_question = req.active_question;
    entry.last_seen = chrono::Utc::now().to_rfc3339();
    if entry.status != "Disqualified" {
        if !req.is_window_focused {
            entry.status = "Flagged".to_string();
        } else {
            entry.status = "Active".to_string();
        }
    }

    (
        StatusCode::OK,
        Json(HeartbeatResponse {
            status: entry.status.clone(),
        }),
    ).into_response()
}

async fn logout_handler(
    State(state): State<AppState>,
    Json(req): Json<LogoutRequest>,
) -> StatusCode {
    let is_prod = state.is_production.load(Ordering::SeqCst);
    if is_prod {
        let live = state.exam_live.read().unwrap();
        if live.is_live {
            if let Some(ref started_str) = live.started_at {
                if let Ok(started_time) = chrono::DateTime::parse_from_rfc3339(started_str) {
                    let total_secs = (live.duration_minutes as i64) * 60;
                    let now = chrono::Utc::now();
                    let elapsed = (now - started_time.with_timezone(&chrono::Utc)).num_seconds();
                    let remaining_secs = (total_secs - elapsed).max(0);
                    if remaining_secs > 900 {
                        eprintln!("[CITADEL SERVER SECURITY] Rejected early logout for '{}': {}s remaining (> 900s). Early exit only allowed in final 15 minutes.", req.candidate_id, remaining_secs);
                        return StatusCode::FORBIDDEN;
                    }
                }
            }
        }
    }

    let mut cands = state.candidates.lock().unwrap();
    let cand = cands.entry(req.candidate_id.clone()).or_insert_with(|| CandidateSession {
        candidate_id: req.candidate_id.clone(),
        name: None,
        ip_address: "127.0.0.1".to_string(),
        active_question: 1,
        violations_count: 0,
        last_seen: chrono::Utc::now().to_rfc3339(),
        status: "Logged Out".to_string(),
        total_score: 0,
        started_at: Some(chrono::Utc::now().to_rfc3339()),
        completed_at: Some(chrono::Utc::now().to_rfc3339()),
    });
    if cand.status != "Disqualified" {
        cand.status = "Logged Out".to_string();
    }
    cand.completed_at = Some(chrono::Utc::now().to_rfc3339());
    cand.last_seen = chrono::Utc::now().to_rfc3339();
    StatusCode::OK
}

#[derive(Serialize, Deserialize)]
pub struct SessionControlResponse {
    pub should_exit: bool,
    pub reason: String,
    pub status: String,
}

#[derive(Deserialize)]
pub struct SessionControlQuery {
    pub candidate_id: Option<String>,
    pub token: Option<String>,
}

async fn client_session_control_handler(
    State(state): State<AppState>,
    Query(q): Query<SessionControlQuery>,
) -> Json<SessionControlResponse> {
    // 1. Check if proctor concluded the whole exam for everyone, OR exam timer expired
    let live = state.exam_live.read().unwrap();
    let total_secs = (live.duration_minutes as i64) * 60;
    let mut exam_over_for_all = !live.is_live && live.ended_at.is_some();

    if live.is_live {
        if let Some(ref started_str) = live.started_at {
            if let Ok(started_time) = chrono::DateTime::parse_from_rfc3339(started_str) {
                let now = chrono::Utc::now();
                let elapsed = (now - started_time.with_timezone(&chrono::Utc)).num_seconds();
                if elapsed >= total_secs {
                    exam_over_for_all = true;
                }
            }
        }
    }

    if exam_over_for_all {
        return Json(SessionControlResponse {
            should_exit: true,
            reason: "Exam concluded for all candidates".to_string(),
            status: "Concluded".to_string(),
        });
    }

    // 2. Resolve candidate ID either from query param or session token
    let target_cid = if let Some(ref cid) = q.candidate_id {
        if !cid.is_empty() { Some(cid.clone()) } else { None }
    } else if let Some(ref tok) = q.token {
        let cand_states = state.candidate_states.lock().unwrap();
        cand_states.iter().find(|(_, c)| c.session_token == *tok).map(|(id, _)| id.clone())
    } else {
        None
    };

    if let Some(ref cid) = target_cid {
        // 1. Check roster status: If revoked or unauthorized, exit immediately
        {
            let roster = state.roster.read().unwrap();
            if !roster.candidates.is_empty() {
                if let Err((_status, code, _msg)) = check_candidate_roster(cid, &roster) {
                    return Json(SessionControlResponse {
                        should_exit: true,
                        reason: format!("Candidate access revoked in roster ({})", code),
                        status: "Disqualified".to_string(),
                    });
                }
            }
        }

        let cands = state.candidates.lock().unwrap();
        let cand_opt = cands.get(cid).or_else(|| {
            cands.iter().find(|(k, _)| k.eq_ignore_ascii_case(cid)).map(|(_, c)| c)
        });

        if let Some(cand) = cand_opt {
            // Disqualified candidates MUST exit immediately and have workstation restored!
            // Regardless of whether in Production or Testing mode and irrespective of remaining exam time,
            // when a candidate is disqualified or removed, release lockdown and remove candidate out immediately.
            if cand.status == "Disqualified" {
                return Json(SessionControlResponse {
                    should_exit: true,
                    reason: "Candidate disqualified: session terminated and workstation unlocked immediately".to_string(),
                    status: "Disqualified".to_string(),
                });
            }

            // Normal submitted candidate who completed session within permitted window:
            if cand.status == "Submitted" || cand.status == "Logged Out" {
                return Json(SessionControlResponse {
                    should_exit: true,
                    reason: "Candidate session concluded".to_string(),
                    status: cand.status.clone(),
                });
            }

            return Json(SessionControlResponse {
                should_exit: false,
                reason: "".to_string(),
                status: cand.status.clone(),
            });
        }
    }

    // 3. General workstation status (exam is still live)
    Json(SessionControlResponse {
        should_exit: false,
        reason: "".to_string(),
        status: "Active".to_string(),
    })
}

async fn kill_all_lockdown_handler(
    State(state): State<AppState>,
    Json(req): Json<LogoutRequest>,
) -> StatusCode {
    let is_prod = state.is_production.load(Ordering::SeqCst);
    if is_prod {
        // Enforce 15-minute early exit restriction in Production Mode:
        let live = state.exam_live.read().unwrap();
        if live.is_live {
            if let Some(ref started_str) = live.started_at {
                if let Ok(started_time) = chrono::DateTime::parse_from_rfc3339(started_str) {
                    let total_secs = (live.duration_minutes as i64) * 60;
                    let now = chrono::Utc::now();
                    let elapsed = (now - started_time.with_timezone(&chrono::Utc)).num_seconds();
                    let remaining_secs = (total_secs - elapsed).max(0);
                    if remaining_secs > 900 {
                        eprintln!(
                            "[CITADEL SERVER SECURITY] Rejected early exit attempt for '{}': {} seconds remaining (> 900s). Early exit only allowed in final 15 minutes.",
                            req.candidate_id, remaining_secs
                        );
                        return StatusCode::FORBIDDEN;
                    }
                }
            }
        }
    }

    eprintln!(
        "[CITADEL SERVER] Candidate '{}' concluded session. Reason: {}",
        req.candidate_id, req.reason.as_deref().unwrap_or("No reason specified")
    );
    {
        let mut cands = state.candidates.lock().unwrap();
        let cand = cands.entry(req.candidate_id.clone()).or_insert_with(|| CandidateSession {
            candidate_id: req.candidate_id.clone(),
            name: None,
            ip_address: "127.0.0.1".to_string(),
            active_question: 1,
            violations_count: 0,
            last_seen: chrono::Utc::now().to_rfc3339(),
            status: "Logged Out".to_string(),
            total_score: 0,
            started_at: Some(chrono::Utc::now().to_rfc3339()),
            completed_at: Some(chrono::Utc::now().to_rfc3339()),
        });
        if cand.status != "Disqualified" {
            cand.status = "Logged Out".to_string();
        }
        cand.completed_at = Some(chrono::Utc::now().to_rfc3339());
        cand.last_seen = chrono::Utc::now().to_rfc3339();
    }

    StatusCode::OK
}


async fn client_handshake_handler(
    State(state): State<AppState>,
    Json(payload): Json<ClientHandshakeRequest>,
) -> Result<Json<ClientHandshakeResponse>, (StatusCode, Json<ClientHandshakeResponse>)> {
    let now = chrono::Utc::now();
    let is_prod = state.is_production.load(Ordering::SeqCst);
    let is_elevated = payload.is_elevated.unwrap_or(false);
    let has_proof = payload.elevation_proof.as_ref().map(|p| p.starts_with("citadel-elevated-")).unwrap_or(false);

    // In Production Mode, mandate that the client must be verified elevated with valid proof token.
    // Degraded or unprivileged execution is strictly blocked by Citadel Security Policy (Finding A).
    if is_prod && (!is_elevated || !has_proof) {
        eprintln!("[CITADEL SERVER SECURITY ALERT] Handshake rejected: Client failed elevation proof verification in Production Mode.");
        return Err((
            StatusCode::FORBIDDEN,
            Json(ClientHandshakeResponse {
                status: "elevation_required".to_string(),
                session_token: String::new(),
                is_production: is_prod,
                server_time: now.to_rfc3339(),
            }),
        ));
    }

    let token = format!(
        "citadel-sess-{:x}{:x}",
        now.timestamp_nanos_opt().unwrap_or(0),
        std::process::id() as u64 ^ 0x5a5a5a5a
    );

    let token_session = TokenSession {
        token: token.clone(),
        client_version: payload.client_version,
        machine_guid: payload.machine_guid,
        created_at: now,
    };

    let mut tokens = state.authorized_tokens.lock().unwrap();
    tokens.insert(token.clone(), token_session);

    eprintln!(
        "[CITADEL SERVER] Secure client handshake verified! Elevated: {}, Issued session token: {}",
        is_elevated, token
    );

    Ok(Json(ClientHandshakeResponse {
        status: "authorized".to_string(),
        session_token: token,
        is_production: is_prod,
        server_time: now.to_rfc3339(),
    }))
}

async fn get_admin_mode_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Result<Json<AdminModeResponse>, StatusCode> {
    if !is_admin_authorized(&headers, &query, &state) {
        return Err(StatusCode::FORBIDDEN);
    }
    let is_prod = state.is_production.load(Ordering::SeqCst);
    let count = state.authorized_tokens.lock().unwrap().len();
    Ok(Json(AdminModeResponse {
        is_production: is_prod,
        mode: if is_prod { "Production (Lockdown Enforced)".to_string() } else { "Testing (Open Access)".to_string() },
        active_authorized_tokens: count,
    }))
}

async fn toggle_admin_mode_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Result<Json<AdminModeResponse>, StatusCode> {
    if !is_admin_authorized(&headers, &query, &state) {
        return Err(StatusCode::FORBIDDEN);
    }
    let prev = state.is_production.load(Ordering::SeqCst);
    let new_val = !prev;
    state.is_production.store(new_val, Ordering::SeqCst);
    let count = state.authorized_tokens.lock().unwrap().len();
    eprintln!("[CITADEL SERVER] Administrator toggled server security mode to: {}", if new_val { "PRODUCTION" } else { "TESTING" });
    Ok(Json(AdminModeResponse {
        is_production: new_val,
        mode: if new_val { "Production (Lockdown Enforced)".to_string() } else { "Testing (Open Access)".to_string() },
        active_authorized_tokens: count,
    }))
}

async fn set_admin_mode_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
    Json(body): Json<SetModeRequest>,
) -> Result<Json<AdminModeResponse>, StatusCode> {
    if !is_admin_authorized(&headers, &query, &state) {
        return Err(StatusCode::FORBIDDEN);
    }
    state.is_production.store(body.production, Ordering::SeqCst);
    let count = state.authorized_tokens.lock().unwrap().len();
    eprintln!("[CITADEL SERVER] Administrator set server security mode to: {}", if body.production { "PRODUCTION" } else { "TESTING" });
    Ok(Json(AdminModeResponse {
        is_production: body.production,
        mode: if body.production { "Production (Lockdown Enforced)".to_string() } else { "Testing (Open Access)".to_string() },
        active_authorized_tokens: count,
    }))
}

async fn client_end_exam_handler(
    State(state): State<AppState>,
    Json(req): Json<LogoutRequest>,
) -> StatusCode {
    kill_all_lockdown_handler(State(state), Json(req)).await
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
        viols.push(event.clone());

        let mut cands = state.candidates.lock().unwrap();
        let cand = cands.entry(req.candidate_id.clone()).or_insert_with(|| CandidateSession {
            candidate_id: req.candidate_id.clone(),
            name: None,
            ip_address: "127.0.0.1".to_string(),
            active_question: 1,
            violations_count: 0,
            last_seen: chrono::Utc::now().to_rfc3339(),
            status: "Active".to_string(),
            total_score: 0,
            started_at: Some(chrono::Utc::now().to_rfc3339()),
            completed_at: None,
        });

        cand.violations_count += 1;
        if cand.status != "Disqualified" {
            cand.status = "Flagged".to_string();
        }
        cand.last_seen = chrono::Utc::now().to_rfc3339();
    }

    {
        let mut c_states = state.candidate_states.lock().unwrap();
        if let Some(cand_st) = c_states.get_mut(&req.candidate_id) {
            cand_st.violations_count += 1;
            if cand_st.status != "Disqualified" {
                cand_st.status = "Flagged".to_string();
            }
            cand_st.last_seen = chrono::Utc::now().to_rfc3339();
            let _ = save_candidate_state(&state.state_dir, cand_st);
        }
    }
    let _ = save_violation_snapshot(&state.state_dir, &event.id, &event);

    StatusCode::OK
}

async fn download_client_handler(headers: HeaderMap) -> Result<Response, StatusCode> {
    let host_header = headers
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("172.60.10.12:8443");

    let candidates = [
        "citadel-client.exe",
        "target/release/citadel-client.exe",
        "target/debug/citadel-client.exe",
        "../citadel-client.exe",
        "../target/release/citadel-client.exe",
        "../target/debug/citadel-client.exe",
        r"\\wsl.localhost\Ubuntu\home\amitlinux\DevProjects\citadel-design\citadel-client.exe",
        r"\\wsl.localhost\Ubuntu\home\amitlinux\DevProjects\citadel-design\target\release\citadel-client.exe",
        r"\\wsl.localhost\Ubuntu\home\amitlinux\DevProjects\citadel-design\target\debug\citadel-client.exe",
        "/home/amitlinux/DevProjects/citadel-design/citadel-client.exe",
        "/home/amitlinux/DevProjects/citadel-design/target/release/citadel-client.exe",
        "/home/amitlinux/DevProjects/citadel-design/target/debug/citadel-client.exe",
    ];

    for path in &candidates {
        if let Ok(mut bytes) = std::fs::read(path) {
            // Append dynamic PE overlay trailer with server host endpoint
            let config_trailer = format!(
                "\n---CITADEL_CONFIG_START---\nENDPOINT={}\n---CITADEL_CONFIG_END---\n",
                host_header.trim()
            );
            bytes.extend_from_slice(config_trailer.as_bytes());

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
) -> Response {
    if !is_admin_authorized(&headers, &query, &state) {
        return StatusCode::FORBIDDEN.into_response();
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
    let disconnected_candidates = candidates.iter().filter(|c| c.status == "Disconnected").count();

    let error_submissions_count = subs_lock
        .iter()
        .filter(|s| s.status != "Accepted")
        .count();
    let passed_submissions_count = subs_lock
        .iter()
        .filter(|s| s.status == "Accepted")
        .count();

    (
        [
            (axum::http::header::CACHE_CONTROL, "no-store, no-cache, must-revalidate, max-age=0"),
            (axum::http::header::PRAGMA, "no-cache"),
        ],
        Json(ProctorDashboardData {
            total_candidates,
            active_candidates,
            flagged_candidates,
            logged_out_candidates,
            disqualified_candidates,
            disconnected_candidates,
            total_submissions: subs_lock.len(),
            error_submissions_count,
            passed_submissions_count,
            candidates,
            recent_violations: viols_lock.iter().rev().take(50).cloned().collect(),
            recent_submissions: subs_lock.iter().rev().take(50).cloned().collect(),
            questions: questions_lock.clone(),
            exam_live: exam_live_lock.clone(),
            is_production: state.is_production.load(Ordering::SeqCst),
        }),
    ).into_response()
}

async fn proctor_metrics_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Response {
    if !is_admin_authorized(&headers, &query, &state) {
        return StatusCode::FORBIDDEN.into_response();
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

    let mut actual_id = id.clone();
    let mut found = false;

    let mut cands = state.candidates.lock().unwrap();
    if let Some(cand) = cands.get_mut(&id) {
        cand.status = "Disqualified".to_string();
        cand.last_seen = chrono::Utc::now().to_rfc3339();
        found = true;
    } else if let Some((k, cand)) = cands.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(&id)) {
        cand.status = "Disqualified".to_string();
        cand.last_seen = chrono::Utc::now().to_rfc3339();
        actual_id = k.clone();
        found = true;
    }

    if found {
        let mut c_states = state.candidate_states.lock().unwrap();
        if let Some(cand_st) = c_states.get_mut(&actual_id) {
            cand_st.status = "Disqualified".to_string();
            cand_st.last_seen = chrono::Utc::now().to_rfc3339();
            let _ = save_candidate_state(&state.state_dir, cand_st);
        }

        // Candidate will receive Disqualified on their next heartbeat or session-control poll
        // and will exit and restore their laptop immediately in both Production and Testing modes.
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


async fn admin_candidate_profile_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<CandidateProfileResponse>, StatusCode> {
    if !is_admin_authorized(&headers, &query, &state) {
        return Err(StatusCode::FORBIDDEN);
    }

    let cand = {
        let cands = state.candidates.lock().unwrap();
        match cands.get(&id) {
            Some(c) => c.clone(),
            None => return Err(StatusCode::NOT_FOUND),
        }
    };

    let questions_map: HashMap<String, String> = {
        let questions = state.questions.read().unwrap();
        questions.iter().map(|q| (q.id.clone(), q.title.clone())).collect()
    };

    let candidate_subs: Vec<CandidateSubmissionDetail> = {
        let subs = state.submissions.lock().unwrap();
        subs.iter()
            .filter(|s| s.candidate_id == id)
            .map(|s| {
                let q_title = questions_map
                    .get(&s.question_id)
                    .cloned()
                    .unwrap_or_else(|| format!("Problem {}", s.question_id));
                CandidateSubmissionDetail {
                    submission_id: s.submission_id.clone(),
                    candidate_id: s.candidate_id.clone(),
                    question_id: s.question_id.clone(),
                    question_title: q_title,
                    language: s.language.clone(),
                    passed_cases: s.passed_cases,
                    total_cases: s.total_cases,
                    score: s.score,
                    status: s.status.clone(),
                    runtime_ms: s.runtime_ms,
                    timestamp: s.timestamp.clone(),
                }
            })
            .collect()
    };

    let candidate_viols: Vec<IntegrityEvent> = {
        let viols = state.violations.lock().unwrap();
        viols.iter()
            .filter(|v| v.candidate_id == id)
            .cloned()
            .collect()
    };

    let passed_subs = candidate_subs
        .iter()
        .filter(|s| s.status == "Accepted" || (s.passed_cases == s.total_cases && s.total_cases > 0))
        .count();

    Ok(Json(CandidateProfileResponse {
        candidate_id: cand.candidate_id,
        name: cand.name,
        ip_address: cand.ip_address,
        status: cand.status,
        total_score: cand.total_score,
        active_question: cand.active_question,
        violations_count: cand.violations_count,
        started_at: cand.started_at,
        completed_at: cand.completed_at,
        last_seen: cand.last_seen,
        total_submissions: candidate_subs.len(),
        passed_submissions: passed_subs,
        total_violations: candidate_viols.len(),
        submissions: candidate_subs,
        integrity_events: candidate_viols,
    }))
}

async fn admin_delete_question_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> StatusCode {
    if !is_admin_authorized(&headers, &query, &state) {
        return StatusCode::FORBIDDEN;
    }

    let mut questions = state.questions.write().unwrap();
    if let Some(pos) = questions.iter().position(|q| q.id == id) {
        questions.remove(pos);
        for (idx, q) in questions.iter_mut().enumerate() {
            q.number = (idx + 1) as u32;
        }
        StatusCode::OK
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

async fn architecture_handler() -> Redirect {
    Redirect::temporary("/static/citadel-architecture.html")
}

async fn serve_static_handler(axum::extract::Path(path): axum::extract::Path<String>) -> Response {
    let clean_path = path.trim_start_matches('/');
    let content_type = if clean_path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if clean_path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if clean_path.ends_with(".woff2") {
        "font/woff2"
    } else if clean_path.ends_with(".woff") {
        "font/woff"
    } else if clean_path.ends_with(".ttf") {
        "font/ttf"
    } else if clean_path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else {
        "application/octet-stream"
    };

    let candidates = [
        PathBuf::from("citadel-server/static").join(clean_path),
        PathBuf::from("static").join(clean_path),
        PathBuf::from("../static").join(clean_path),
    ];

    for file_path in &candidates {
        if file_path.exists() && file_path.is_file() {
            if let Ok(bytes) = std::fs::read(file_path) {
                return (
                    StatusCode::OK,
                    [
                        (axum::http::header::CONTENT_TYPE, content_type),
                        (axum::http::header::CACHE_CONTROL, "public, max-age=31536000"),
                    ],
                    bytes,
                ).into_response();
            }
        }
    }

    match clean_path {
        "ace.bundle.js" => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/javascript; charset=utf-8")],
            include_bytes!("../static/ace.bundle.js").as_slice(),
        ).into_response(),
        "citadel-skin.css" => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "text/css; charset=utf-8")],
            include_bytes!("../static/citadel-skin.css").as_slice(),
        ).into_response(),
        "fonts/citadel-fonts.css" => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "text/css; charset=utf-8")],
            include_bytes!("../static/fonts/citadel-fonts.css").as_slice(),
        ).into_response(),
        "fonts/Geist-Variable.woff2" => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "font/woff2")],
            include_bytes!("../static/fonts/Geist-Variable.woff2").as_slice(),
        ).into_response(),
        "fonts/GeistMono-Variable.woff2" => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "font/woff2")],
            include_bytes!("../static/fonts/GeistMono-Variable.woff2").as_slice(),
        ).into_response(),
        "fonts/InstrumentSerif-Regular.woff2" => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "font/woff2")],
            include_bytes!("../static/fonts/InstrumentSerif-Regular.woff2").as_slice(),
        ).into_response(),
        "fonts/InstrumentSerif-Italic.woff2" => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "font/woff2")],
            include_bytes!("../static/fonts/InstrumentSerif-Italic.woff2").as_slice(),
        ).into_response(),
        _ => (StatusCode::NOT_FOUND, "Static file not found").into_response(),
    }
}


// ============================================================================
// STATE PERSISTENCE & ROSTER AUTHENTICATION HANDLERS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateLoginRequest {
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub passcode: Option<String>,
    #[serde(default)]
    pub roll_number: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    pub client_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateLoginResponse {
    pub status: String,
    pub session_token: String,
    pub candidate_id: String,
    pub name: String,
    pub is_production: bool,
    pub resume_state: Option<CandidateResumeState>,
    pub server_time: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExamPasscodeResponse {
    pub passcode: String,
}

#[derive(Debug, Deserialize)]
pub struct SetPasscodePayload {
    pub passcode: String,
}

async fn admin_get_passcode_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Result<Json<ExamPasscodeResponse>, StatusCode> {
    if !is_admin_authorized(&headers, &query, &state) {
        return Err(StatusCode::FORBIDDEN);
    }
    let p = state.exam_passcode.read().unwrap();
    Ok(Json(ExamPasscodeResponse { passcode: p.clone() }))
}

async fn admin_set_passcode_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
    Json(payload): Json<SetPasscodePayload>,
) -> Result<Json<ExamPasscodeResponse>, StatusCode> {
    if !is_admin_authorized(&headers, &query, &state) {
        return Err(StatusCode::FORBIDDEN);
    }
    let trimmed = payload.passcode.trim();
    if trimmed.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut p = state.exam_passcode.write().unwrap();
    *p = trimmed.to_string();
    eprintln!("[CITADEL SERVER] Administrator updated exam entry passcode to: {}", *p);
    Ok(Json(ExamPasscodeResponse { passcode: p.clone() }))
}


pub fn check_candidate_roster(
    candidate_id: &str,
    roster: &crate::persistence::ExamRoster,
) -> Result<crate::persistence::RosterEntry, (StatusCode, &'static str, String)> {
    let cid = candidate_id.trim();
    if cid.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "MISSING_IDENTIFIER",
            "Candidate Roll Number or Email Address is required to sign in.".to_string(),
        ));
    }

    if roster.candidates.is_empty() {
        return Err((
            StatusCode::FORBIDDEN,
            "ROSTER_EMPTY",
            "Assessment roster is empty. No candidates are enrolled for this examination session. Please contact the exam administrator.".to_string(),
        ));
    }

    let entry_opt = roster.candidates.iter().find(|c| {
        c.email.eq_ignore_ascii_case(cid)
            || c.roll_number.as_deref().map(|r| r.eq_ignore_ascii_case(cid)).unwrap_or(false)
            || c.email.split('@').next().map(|u| u.eq_ignore_ascii_case(cid)).unwrap_or(false)
    });

    match entry_opt {
        None => Err((
            StatusCode::FORBIDDEN,
            "ROSTER_NOT_FOUND",
            format!("Candidate ID / Email '{}' is not registered in the exam roster. Access denied.", cid),
        )),
        Some(entry) => {
            if !entry.allowed {
                Err((
                    StatusCode::FORBIDDEN,
                    "ACCESS_REVOKED",
                    format!("Candidature for '{}' has been revoked or excluded from this exam by the proctor.", cid),
                ))
            } else {
                Ok(entry.clone())
            }
        }
    }
}

async fn candidate_login_handler(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(payload): Json<CandidateLoginRequest>,
) -> Response {
    // OA Entry point: email (or roll_number for backwards compat)
    let candidate_id = payload.email
        .or(payload.roll_number)
        .unwrap_or_default()
        .trim()
        .to_string();

    if candidate_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "MISSING_IDENTIFIER",
                "message": "Candidate Email Address is required to sign in."
            })),
        ).into_response();
    }

    // 1. Verify Passcode (Mandatory single exam passcode for the assessment batch)
    let active_passcode = state.exam_passcode.read().unwrap().clone();
    let candidate_passcode = payload.passcode.as_deref().unwrap_or("").trim();

    // Verify passcode against exam passcode OR master admin key
    if candidate_passcode.is_empty() || (candidate_passcode != active_passcode.trim() && candidate_passcode != state.admin_key.trim()) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "INVALID_PASSCODE",
                "message": "Invalid Exam Passcode. Please enter the pre-decided exam passcode announced by your proctor."
            })),
        ).into_response();
    }

    let is_prod = state.is_production.load(Ordering::SeqCst);
    let now = chrono::Utc::now();

    // Device check: In Production Mode, exams can only be taken from a laptop/desktop workstation
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    if is_prod && is_mobile_or_tablet_user_agent(user_agent) {
        eprintln!(
            "[CITADEL DEVICE SECURITY] Candidate login REJECTED for '{}' from mobile/tablet device ('{}') in Production Mode",
            candidate_id, user_agent
        );
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "DEVICE_DISALLOWED",
                "message": "This exam needs to be taken from a laptop. Mobile devices and tablets are not permitted in production mode."
            })),
        ).into_response();
    }

    // 2. Check Exam Status
    {
        let live = state.exam_live.read().unwrap();
        if !live.is_live && live.ended_at.is_some() {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "EXAM_CONCLUDED",
                    "message": "The assessment session has already been concluded by the proctor."
                })),
            ).into_response();
        }
    }

    // 3. Validate against Roster (strictly enforced in BOTH testing and production)
    let mut resolved_display_name = payload.name.clone().unwrap_or_else(|| candidate_id.clone());
    {
        let roster = state.roster.read().unwrap();
        match check_candidate_roster(&candidate_id, &roster) {
            Err((status, code, msg)) => {
                eprintln!("[CITADEL ROSTER SECURITY] Candidate login REJECTED for '{}': {} ({})", candidate_id, code, msg);
                return (
                    status,
                    Json(serde_json::json!({
                        "error": code,
                        "message": msg
                    })),
                ).into_response();
            }
            Ok(entry) => {
                if !entry.name.is_empty() {
                    resolved_display_name = entry.name.clone();
                }
            }
        }
    }

    // 4. Issue or preserve session token
    let token = format!(
        "citadel-sess-{:x}{:x}",
        now.timestamp_nanos_opt().unwrap_or(0),
        std::process::id() as u64 ^ 0x3c3c3c3c
    );

    let token_session = TokenSession {
        token: token.clone(),
        client_version: payload.client_version.unwrap_or_else(|| "portal-web".to_string()),
        machine_guid: None,
        created_at: now,
    };
    state.authorized_tokens.lock().unwrap().insert(token.clone(), token_session);

    // 5. Candidate state lookup & resumption calculation
    let mut cand_states = state.candidate_states.lock().unwrap();
    let mut cands = state.candidates.lock().unwrap();

    let client_ip = headers.get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("127.0.0.1").trim().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    if let Some(existing) = cand_states.get_mut(&candidate_id) {
        // Disqualified check
        if existing.status == "Disqualified" {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": "DISQUALIFIED",
                    "message": "Candidate has been disqualified by the proctor due to security violations."
                })),
            ).into_response();
        }

        // Single session conflict check:
        if existing.status == "Active" && existing.ip_address != client_ip {
            if let Ok(last) = chrono::DateTime::parse_from_rfc3339(&existing.last_seen) {
                let diff_secs = (now - last.with_timezone(&chrono::Utc)).num_seconds();
                if diff_secs < 15 {
                    return (
                        StatusCode::CONFLICT,
                        Json(serde_json::json!({
                            "error": "ALREADY_ACTIVE",
                            "message": format!("Candidate session for '{}' is already active on another workstation (IP: {}).", candidate_id, existing.ip_address)
                        })),
                    ).into_response();
                }
            }
        }

        // Resume session: calculate paused time if disconnected
        if let Some(ref paused_at_str) = existing.timer_paused_at {
            if let Ok(paused_time) = chrono::DateTime::parse_from_rfc3339(paused_at_str) {
                let elapsed = (now - paused_time.with_timezone(&chrono::Utc)).num_seconds().max(0) as u64;
                existing.remaining_seconds = existing.remaining_seconds.saturating_sub(elapsed);
            }
            existing.timer_paused_at = None;
        }

        existing.status = "Active".to_string();
        existing.last_seen = now.to_rfc3339();
        existing.ip_address = client_ip.clone();
        existing.session_token = token.clone();

        // Update in-memory candidates map
        if let Some(cand_sess) = cands.get_mut(&candidate_id) {
            cand_sess.status = "Active".to_string();
            cand_sess.last_seen = now.to_rfc3339();
            cand_sess.ip_address = client_ip.clone();
            cand_sess.name = Some(existing.name.clone());
        }

        // Persist to disk
        let _ = save_candidate_state(&state.state_dir, existing);

        let resume_state = existing.to_resume_state();
        let display_name = existing.name.clone();

        let mut resp = Json(CandidateLoginResponse {
            status: "resumed".to_string(),
            session_token: token.clone(),
            candidate_id: candidate_id.clone(),
            name: display_name,
            is_production: is_prod,
            resume_state: Some(resume_state),
            server_time: now.to_rfc3339(),
        }).into_response();

        let cookie_val = format!("citadel_auth_token={}; Path=/; SameSite=Lax; Max-Age=28800", token);
        if let Ok(v) = cookie_val.parse() {
            resp.headers_mut().insert(header::SET_COOKIE, v);
        }
        let cand_cookie = format!("citadel_candidate_id={}; Path=/; SameSite=Lax; Max-Age=28800", candidate_id);
        if let Ok(v) = cand_cookie.parse() {
            resp.headers_mut().append(header::SET_COOKIE, v);
        }
        return resp;
    }

    // New candidate session
    let initial_seconds = {
        let live = state.exam_live.read().unwrap();
        (live.duration_minutes as u64) * 60
    };

    let new_state = CandidateState {
        candidate_id: candidate_id.clone(),
        name: resolved_display_name.clone(),
        ip_address: client_ip.clone(),
        active_question_id: "q1-two-sum".to_string(),
        active_language: "python".to_string(),
        code_store: HashMap::new(),
        remaining_seconds: initial_seconds,
        timer_paused_at: None,
        best_scores: HashMap::new(),
        total_score: 0,
        status: "Active".to_string(),
        violations_count: 0,
        started_at: Some(now.to_rfc3339()),
        last_seen: now.to_rfc3339(),
        completed_at: None,
        session_token: token.clone(),
        state_version: 0,
        last_synced_at: now.to_rfc3339(),
    };

    cands.insert(candidate_id.clone(), CandidateSession {
        candidate_id: candidate_id.clone(),
        name: Some(resolved_display_name.clone()),
        ip_address: client_ip.clone(),
        active_question: 1,
        violations_count: 0,
        last_seen: now.to_rfc3339(),
        status: "Active".to_string(),
        total_score: 0,
        started_at: Some(now.to_rfc3339()),
        completed_at: None,
    });

    cand_states.insert(candidate_id.clone(), new_state.clone());
    let _ = save_candidate_state(&state.state_dir, &new_state);

    let mut resp = Json(CandidateLoginResponse {
        status: "created".to_string(),
        session_token: token.clone(),
        candidate_id: candidate_id.clone(),
        name: resolved_display_name,
        is_production: is_prod,
        resume_state: None,
        server_time: now.to_rfc3339(),
    }).into_response();

    let cookie_val = format!("citadel_auth_token={}; Path=/; SameSite=Lax; Max-Age=28800", token);
    if let Ok(v) = cookie_val.parse() {
        resp.headers_mut().insert(header::SET_COOKIE, v);
    }
    let cand_cookie = format!("citadel_candidate_id={}; Path=/; SameSite=Lax; Max-Age=28800", candidate_id);
    if let Ok(v) = cand_cookie.parse() {
        resp.headers_mut().append(header::SET_COOKIE, v);
    }
    resp
}

#[derive(Debug, Clone, Deserialize)]
pub struct StateSyncRequest {
    pub candidate_id: String,
    pub active_question_id: Option<String>,
    pub active_language: Option<String>,
    pub remaining_seconds: Option<u64>,
    pub code_store: Option<HashMap<String, HashMap<String, String>>>,
    pub state_version: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSyncResponse {
    pub status: String,
    pub candidate_id: String,
    pub state_version: u64,
    pub remaining_seconds: u64,
    pub server_time: String,
}

async fn state_sync_handler(
    State(state): State<AppState>,
    Json(payload): Json<StateSyncRequest>,
) -> Response {
    let now = chrono::Utc::now();
    let cid = payload.candidate_id.trim();
    if cid.is_empty() {
        return StatusCode::BAD_REQUEST.into_response();
    }

    // Verify candidate against roster
    {
        let roster = state.roster.read().unwrap();
        if !roster.candidates.is_empty() {
            if let Err((status, code, msg)) = check_candidate_roster(cid, &roster) {
                return (
                    status,
                    Json(serde_json::json!({
                        "error": code,
                        "message": msg
                    })),
                ).into_response();
            }
        }
    }

    let mut cand_states = state.candidate_states.lock().unwrap();
    let mut cands = state.candidates.lock().unwrap();

    let cand = match cand_states.get_mut(cid) {
        Some(c) => c,
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    if cand.status == "Disqualified" {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "DISQUALIFIED",
                "message": "Candidate is disqualified."
            })),
        ).into_response();
    }

    if let Some(new_codes) = payload.code_store {
        for (qid, lang_map) in new_codes {
            let entry = cand.code_store.entry(qid).or_insert_with(HashMap::new);
            for (lang, code) in lang_map {
                entry.insert(lang, code);
            }
        }
    }

    if let Some(qid) = payload.active_question_id {
        cand.active_question_id = qid;
    }
    if let Some(lang) = payload.active_language {
        cand.active_language = lang;
    }
    if let Some(rem) = payload.remaining_seconds {
        cand.remaining_seconds = rem.min(cand.remaining_seconds);
    }
    cand.state_version = payload.state_version.unwrap_or(cand.state_version + 1);
    cand.last_synced_at = now.to_rfc3339();
    cand.last_seen = now.to_rfc3339();
    if cand.status != "Flagged" && cand.status != "Logged Out" && cand.status != "Submitted" {
        cand.status = "Active".to_string();
    }

    if let Some(sess) = cands.get_mut(cid) {
        sess.last_seen = now.to_rfc3339();
        if let Ok(num) = cand.active_question_id.replace("q-", "").parse::<u32>() {
            sess.active_question = num;
        }
        if sess.status != "Disqualified" && sess.status != "Flagged" && sess.status != "Logged Out" {
            sess.status = "Active".to_string();
        }
    }

    let _ = save_candidate_state(&state.state_dir, cand);

    Json(StateSyncResponse {
        status: "synced".to_string(),
        candidate_id: cid.to_string(),
        state_version: cand.state_version,
        remaining_seconds: cand.remaining_seconds,
        server_time: now.to_rfc3339(),
    }).into_response()
}

#[derive(Debug, Deserialize)]
pub struct RestoreQuery {
    pub candidate_id: String,
}

async fn state_restore_handler(
    Query(q): Query<RestoreQuery>,
    State(state): State<AppState>,
) -> Response {
    let cand_states = state.candidate_states.lock().unwrap();
    if let Some(cand) = cand_states.get(&q.candidate_id) {
        Json(cand.clone()).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn admin_get_roster_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Result<Json<ExamRoster>, StatusCode> {
    if !is_admin_authorized(&headers, &query, &state) {
        return Err(StatusCode::FORBIDDEN);
    }
    let roster = state.roster.read().unwrap();
    Ok(Json(roster.clone()))
}

#[derive(Debug, Deserialize)]
pub struct RosterUploadPayload {
    pub exam_id: Option<String>,
    pub candidates: Option<Vec<RosterEntry>>,
    pub csv_data: Option<String>,
}

async fn admin_upload_roster_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
    body: axum::extract::Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !is_admin_authorized(&headers, &query, &state) {
        return Err(StatusCode::FORBIDDEN);
    }

    let mut new_entries = Vec::new();
    let mut exam_id = "CITADEL-EXAM".to_string();

    if let Ok(upload) = serde_json::from_value::<RosterUploadPayload>(body.0.clone()) {
        if let Some(eid) = upload.exam_id {
            exam_id = eid;
        }
        if let Some(cands) = upload.candidates {
            new_entries = cands;
        } else if let Some(csv) = upload.csv_data {
            if let Ok(parsed) = parse_roster_csv(&csv) {
                new_entries = parsed;
            }
        }
    } else if let Some(arr) = body.0.as_array() {
        if let Ok(cands) = serde_json::from_value::<Vec<RosterEntry>>(serde_json::Value::Array(arr.clone())) {
            new_entries = cands;
        }
    }

    let count = new_entries.len();
    {
        let mut roster = state.roster.write().unwrap();
        roster.exam_id = exam_id;
        roster.created_at = chrono::Utc::now().to_rfc3339();
        roster.candidates = new_entries;
        let _ = save_roster(&state.state_dir, &roster);
    }

    Ok(Json(serde_json::json!({
        "status": "success",
        "registered_count": count
    })))
}

async fn admin_add_roster_candidate_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    State(state): State<AppState>,
    Json(entry): Json<RosterEntry>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !is_admin_authorized(&headers, &query, &state) {
        return Err(StatusCode::FORBIDDEN);
    }

    let target_id = entry.get_identifier().to_string();
    let mut roster = state.roster.write().unwrap();
    if let Some(existing) = roster.candidates.iter_mut().find(|c| {
        c.get_identifier().eq_ignore_ascii_case(&target_id)
            || (entry.roll_number.is_some() && c.roll_number == entry.roll_number)
    }) {
        *existing = entry;
    } else {
        roster.candidates.push(entry);
    }
    let _ = save_roster(&state.state_dir, &roster);

    Ok(Json(serde_json::json!({ "status": "success" })))
}

async fn admin_delete_roster_candidate_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !is_admin_authorized(&headers, &query, &state) {
        return Err(StatusCode::FORBIDDEN);
    }

    let mut roster = state.roster.write().unwrap();
    roster.candidates.retain(|c| {
        !c.get_identifier().eq_ignore_ascii_case(&id)
            && !c.roll_number.as_deref().map(|r| r.eq_ignore_ascii_case(&id)).unwrap_or(false)
    });
    let _ = save_roster(&state.state_dir, &roster);

    Ok(Json(serde_json::json!({ "status": "success" })))
}

#[derive(Debug, Deserialize)]
pub struct ExtendTimePayload {
    pub extra_minutes: Option<u32>,
    pub extra_seconds: Option<u64>,
}

async fn admin_extend_candidate_time_handler(
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<ExtendTimePayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !is_admin_authorized(&headers, &query, &state) {
        return Err(StatusCode::FORBIDDEN);
    }

    let additional_secs = payload.extra_seconds
        .or_else(|| payload.extra_minutes.map(|m| (m as u64) * 60))
        .unwrap_or(600);

    let mut cand_states = state.candidate_states.lock().unwrap();
    if let Some(cand) = cand_states.get_mut(&id) {
        cand.remaining_seconds += additional_secs;
        let _ = save_candidate_state(&state.state_dir, cand);
        Ok(Json(serde_json::json!({
            "status": "success",
            "candidate_id": id,
            "new_remaining_seconds": cand.remaining_seconds
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn disconnect_watchdog_loop(state: AppState) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
    loop {
        interval.tick().await;

        let now = chrono::Utc::now();
        let mut states_to_persist = Vec::new();

        {
            let mut c_states = state.candidate_states.lock().unwrap();
            let mut c_sessions = state.candidates.lock().unwrap();

            for (cid, cand) in c_states.iter_mut() {
                if cand.status == "Active" || cand.status == "Flagged" {
                    if let Ok(last) = chrono::DateTime::parse_from_rfc3339(&cand.last_seen) {
                        let diff_secs = (now - last.with_timezone(&chrono::Utc)).num_seconds();
                        if diff_secs > 15 {
                            cand.status = "Disconnected".to_string();
                            cand.timer_paused_at = Some(now.to_rfc3339());
                            states_to_persist.push(cand.clone());

                            if let Some(sess) = c_sessions.get_mut(cid) {
                                sess.status = "Disconnected".to_string();
                            }
                        }
                    }
                }
            }
        }

        for st in states_to_persist {
            let _ = save_candidate_state(&state.state_dir, &st);
        }
    }
}
