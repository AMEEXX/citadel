use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RosterEntry {
    #[serde(default)]
    pub email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roll_number: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    #[serde(default = "default_true")]
    pub allowed: bool,
}

impl RosterEntry {
    pub fn get_identifier(&self) -> &str {
        if !self.email.is_empty() {
            &self.email
        } else if let Some(ref r) = self.roll_number {
            r
        } else {
            ""
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExamRoster {
    #[serde(default)]
    pub exam_id: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub candidates: Vec<RosterEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateResumeState {
    pub active_question_id: String,
    pub active_language: String,
    pub remaining_seconds: u64,
    pub code_store: HashMap<String, HashMap<String, String>>,
    pub best_scores: HashMap<String, u32>,
    pub total_score: u32,
    pub violations_count: u32,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateState {
    pub candidate_id: String,
    pub name: String,
    pub ip_address: String,
    pub active_question_id: String,
    pub active_language: String,
    pub code_store: HashMap<String, HashMap<String, String>>,
    pub remaining_seconds: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer_paused_at: Option<String>,
    #[serde(default)]
    pub best_scores: HashMap<String, u32>,
    #[serde(default)]
    pub total_score: u32,
    pub status: String,
    #[serde(default)]
    pub violations_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    pub last_seen: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub session_token: String,
    #[serde(default)]
    pub state_version: u64,
    #[serde(default)]
    pub last_synced_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_details: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_timestamp: Option<String>,
    #[serde(default)]
    pub best_passed_cases: HashMap<String, Vec<bool>>,
}

impl CandidateState {
    pub fn to_resume_state(&self) -> CandidateResumeState {
        CandidateResumeState {
            active_question_id: self.active_question_id.clone(),
            active_language: self.active_language.clone(),
            remaining_seconds: self.remaining_seconds,
            code_store: self.code_store.clone(),
            best_scores: self.best_scores.clone(),
            total_score: self.total_score,
            violations_count: self.violations_count,
            status: self.status.clone(),
            exit_reason: self.exit_reason.clone(),
        }
    }
}

pub fn sanitize_filename(id: &str) -> String {
    let replaced = id.replace('@', "_at_");
    let sanitized: String = replaced
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' { c } else { '_' })
        .collect();
    sanitized.replace("..", "__")
}

pub fn ensure_directories(state_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(state_dir)?;
    fs::create_dir_all(state_dir.join("candidates"))?;
    fs::create_dir_all(state_dir.join("submissions"))?;
    fs::create_dir_all(state_dir.join("violations"))?;
    Ok(())
}

fn atomic_write_json<T: Serialize>(path: &Path, data: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("tmp");
    let json_bytes = serde_json::to_vec_pretty(data)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    {
        let mut file = File::create(&tmp_path)?;
        file.write_all(&json_bytes)?;
        file.sync_all()?;
    }

    fs::rename(&tmp_path, path)?;
    Ok(())
}

pub fn save_roster(state_dir: &Path, roster: &ExamRoster) -> io::Result<()> {
    ensure_directories(state_dir)?;
    let roster_path = state_dir.join("roster.json");
    atomic_write_json(&roster_path, roster)
}

pub fn load_roster(state_dir: &Path) -> io::Result<ExamRoster> {
    let roster_path = state_dir.join("roster.json");
    if !roster_path.exists() {
        return Ok(ExamRoster::default());
    }
    let content = fs::read_to_string(&roster_path)?;
    serde_json::from_str(&content).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub fn parse_roster_csv(csv_content: &str) -> Result<Vec<RosterEntry>, String> {
    let mut entries = Vec::new();
    let lines: Vec<&str> = csv_content.lines().collect();
    if lines.is_empty() {
        return Ok(entries);
    }

    let mut start_idx = 0;
    let first = lines[0].to_lowercase();
    if first.contains("roll") || first.contains("id") || first.contains("name") || first.contains("email") {
        start_idx = 1;
    }

    for line in lines[start_idx..].iter() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = trimmed.split(',').map(|s| s.trim().trim_matches('"')).collect();
        if parts.is_empty() || parts[0].is_empty() {
            continue;
        }

        let first_val = parts[0].trim().to_string();
        let (email, roll_number) = if first_val.contains('@') {
            (first_val.to_lowercase(), None)
        } else {
            (first_val.to_lowercase(), Some(first_val.clone()))
        };

        let name = if parts.len() > 1 && !parts[1].is_empty() {
            parts[1].trim().to_string()
        } else {
            first_val.clone()
        };

        let section = if parts.len() > 2 && !parts[2].is_empty() {
            Some(parts[2].trim().to_string())
        } else {
            None
        };

        entries.push(RosterEntry {
            email,
            roll_number,
            name,
            section,
            allowed: true,
        });
    }

    Ok(entries)
}

pub fn save_candidate_state(state_dir: &Path, state: &CandidateState) -> io::Result<()> {
    ensure_directories(state_dir)?;
    let safe_id = sanitize_filename(&state.candidate_id);
    let target = state_dir.join("candidates").join(format!("{}.json", safe_id));
    atomic_write_json(&target, state)
}

pub fn load_candidate_state(state_dir: &Path, candidate_id: &str) -> io::Result<Option<CandidateState>> {
    let safe_id = sanitize_filename(candidate_id);
    let target = state_dir.join("candidates").join(format!("{}.json", safe_id));
    if !target.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&target)?;
    let state: CandidateState = serde_json::from_str(&content)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(Some(state))
}

pub fn load_all_candidate_states(state_dir: &Path) -> io::Result<HashMap<String, CandidateState>> {
    let mut map = HashMap::new();
    let candidates_dir = state_dir.join("candidates");
    if !candidates_dir.exists() {
        return Ok(map);
    }

    for entry in fs::read_dir(candidates_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(state) = serde_json::from_str::<CandidateState>(&content) {
                    map.insert(state.candidate_id.clone(), state);
                }
            }
        }
    }

    Ok(map)
}

pub fn save_submission_snapshot<T: Serialize>(state_dir: &Path, submission_id: &str, sub: &T) -> io::Result<()> {
    ensure_directories(state_dir)?;
    let safe_id = sanitize_filename(submission_id);
    let target = state_dir.join("submissions").join(format!("{}.json", safe_id));
    atomic_write_json(&target, sub)
}

pub fn save_violation_snapshot<T: Serialize>(state_dir: &Path, event_id: &str, evt: &T) -> io::Result<()> {
    ensure_directories(state_dir)?;
    let safe_id = sanitize_filename(event_id);
    let target = state_dir.join("violations").join(format!("{}.json", safe_id));
    atomic_write_json(&target, evt)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filename_sanitization() {
        assert_eq!(sanitize_filename("21BCS001"), "21BCS001");
        assert_eq!(sanitize_filename("alice.walker@univ.edu"), "alice.walker_at_univ.edu");
        assert_eq!(sanitize_filename("../evil/path"), "___evil_path");
        assert_eq!(sanitize_filename("cand-id_99"), "cand-id_99");
    }

    #[test]
    fn test_parse_roster_csv() {
        let csv = r#"roll_number,name,section
21BCS001,"Amit Kumar",A
21BCS002,Priya Sharma,B
# this is a comment
21BCS003,John Doe
"#;
        let entries = parse_roster_csv(csv).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].roll_number.as_deref(), Some("21BCS001"));
        assert_eq!(entries[0].email, "21bcs001");
        assert_eq!(entries[0].name, "Amit Kumar");
        assert_eq!(entries[0].section, Some("A".to_string()));
        assert!(entries[0].allowed);
        assert_eq!(entries[2].name, "John Doe");
        assert_eq!(entries[2].section, None);
    }

    #[test]
    fn test_state_persistence_lifecycle() {
        let temp_dir = std::env::temp_dir().join(format!("citadel_test_{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)));
        let _ = fs::create_dir_all(&temp_dir);

        // Test roster save/load
        let roster = ExamRoster {
            exam_id: "CS101".to_string(),
            created_at: "2026-09-28".to_string(),
            candidates: vec![
                RosterEntry {
                    email: "alice@univ.edu".to_string(),
                    roll_number: Some("ROLL001".to_string()),
                    name: "Alice Smith".to_string(),
                    section: Some("1".to_string()),
                    allowed: true,
                }
            ],
        };
        save_roster(&temp_dir, &roster).unwrap();
        let loaded_roster = load_roster(&temp_dir).unwrap();
        assert_eq!(loaded_roster.candidates.len(), 1);
        assert_eq!(loaded_roster.candidates[0].name, "Alice Smith");

        // Test candidate state save/load
        let mut code_map = HashMap::new();
        let mut py_code = HashMap::new();
        py_code.insert("python".to_string(), "print(123)".to_string());
        code_map.insert("q-001".to_string(), py_code);

        let cand_state = CandidateState {
            candidate_id: "ROLL001".to_string(),
            name: "Alice Smith".to_string(),
            ip_address: "127.0.0.1".to_string(),
            active_question_id: "q-001".to_string(),
            active_language: "python".to_string(),
            code_store: code_map,
            remaining_seconds: 3600,
            timer_paused_at: None,
            best_scores: HashMap::new(),
            total_score: 50,
            status: "Active".to_string(),
            violations_count: 0,
            started_at: Some("2026-09-28T10:00:00Z".to_string()),
            last_seen: "2026-09-28T10:15:00Z".to_string(),
            completed_at: None,
            session_token: "tok-123".to_string(),
            state_version: 1,
            last_synced_at: "2026-09-28T10:15:00Z".to_string(),
            exit_reason: None,
            exit_details: None,
            exit_timestamp: None,
            best_passed_cases: HashMap::new(),
        };

        save_candidate_state(&temp_dir, &cand_state).unwrap();
        let loaded_cand = load_candidate_state(&temp_dir, "ROLL001").unwrap().expect("must exist");
        assert_eq!(loaded_cand.candidate_id, "ROLL001");
        assert_eq!(loaded_cand.total_score, 50);
        assert_eq!(loaded_cand.code_store.get("q-001").unwrap().get("python").unwrap(), "print(123)");

        let all = load_all_candidate_states(&temp_dir).unwrap();
        assert_eq!(all.len(), 1);
        assert!(all.contains_key("ROLL001"));

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
