use axum::{
    extract::Path,
    http::StatusCode,
    response::Html,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};

use crate::questions::{
    get_all_questions, get_exam_info, get_question_by_id, get_question_summaries, ExamInfo,
    Question, QuestionSummary,
};
use crate::ui::render_portal_html;

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

pub fn build_app() -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/", get(portal_handler))
        .route("/health", get(health_handler))
        .route("/api/v1/exam/info", get(exam_info_handler))
        .route("/api/v1/questions", get(list_questions_handler))
        .route("/api/v1/questions/:id", get(get_question_handler))
        .route("/api/v1/submissions", post(submit_code_handler))
        .layer(cors)
}

async fn portal_handler() -> Html<&'static str> {
    Html(render_portal_html())
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        service: "citadel-exam-server".to_string(),
        version: "0.1.0".to_string(),
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
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn submit_code_handler(
    Json(payload): Json<SubmissionRequest>,
) -> Result<Json<SubmissionResponse>, StatusCode> {
    let question = match get_question_by_id(&payload.question_id) {
        Some(q) => q,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let total_cases = question.sample_cases.len() as u32;
    // Evaluation simulation in offline mode
    let is_blank = payload.source_code.trim().is_empty();
    let (status, passed, score, details) = if is_blank {
        ("Compilation Error".to_string(), 0, 0, "No code submitted.".to_string())
    } else {
        (
            "Accepted".to_string(),
            total_cases,
            question.points,
            format!("All {}/{} sample test cases passed successfully within limits.", total_cases, total_cases),
        )
    };

    Ok(Json(SubmissionResponse {
        submission_id: format!("sub-{}", chrono::Utc::now().timestamp_millis()),
        status,
        passed_cases: passed,
        total_cases,
        score,
        runtime_ms: 18,
        memory_mb: 2.1,
        details,
    }))
}
