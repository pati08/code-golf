use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub is_admin: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Problem {
    pub id: i64,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub difficulty: String,
    pub is_published: bool,
    pub time_limit_ms: i64,
    pub memory_limit_kb: i64,
    pub created_by: i64,
    pub created_at: String,
    pub updated_at: String,
    pub par_solution: Option<String>,
    pub par_byte_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TestCase {
    pub id: i64,
    pub problem_id: i64,
    pub input: String,
    pub expected_output: String,
    pub is_sample: bool,
    pub ordinal: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Language {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub file_extension: String,
    pub run_command: String,
    pub is_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Submission {
    pub id: i64,
    pub user_id: i64,
    pub problem_id: i64,
    pub language_id: i64,
    pub code: String,
    pub byte_count: i64,
    pub status: String,
    pub error_output: Option<String>,
    pub created_at: String,
    pub judged_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BestSubmission {
    pub user_id: i64,
    pub problem_id: i64,
    pub language_id: i64,
    pub submission_id: i64,
    pub byte_count: i64,
}

/// Joined struct for scoreboard display
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ScoreboardEntry {
    pub username: String,
    pub total_bytes: i64,
    pub solved_count: i64,
}

/// Joined struct for per-problem scoreboard
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProblemScoreEntry {
    pub username: String,
    pub language_name: String,
    pub byte_count: i64,
    pub submitted_at: String,
}

/// Submission with joined info for display
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SubmissionDetail {
    pub id: i64,
    pub username: String,
    pub problem_title: String,
    pub problem_slug: String,
    pub language_name: String,
    pub byte_count: i64,
    pub status: String,
    pub error_output: Option<String>,
    pub created_at: String,
    pub judged_at: Option<String>,
}
