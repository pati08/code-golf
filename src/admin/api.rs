/// JSON API handlers authenticated via `Authorization: Bearer <token>`.
/// All routes are mounted under `/api/admin/`.
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::{app::AppState, auth::BearerAdmin};

// ── Error helpers ─────────────────────────────────────────────────────────────

type ApiError = (StatusCode, Json<serde_json::Value>);
type ApiResult<T> = Result<(StatusCode, Json<T>), ApiError>;

fn api_err(status: StatusCode, msg: &str) -> ApiError {
    (status, Json(serde_json::json!({ "error": msg })))
}

// ── POST /api/admin/problems ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateProblemBody {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub difficulty: String,
    pub time_limit_ms: i64,
    pub memory_limit_kb: i64,
    pub par_solution: Option<String>,
    pub tournament_id: Option<i64>,
}

#[derive(Serialize)]
pub struct ProblemCreated {
    pub id: i64,
    pub slug: String,
}

pub async fn post_api_create_problem(
    State(state): State<AppState>,
    BearerAdmin(admin): BearerAdmin,
    Json(body): Json<CreateProblemBody>,
) -> ApiResult<ProblemCreated> {
    let par_solution = body
        .par_solution
        .as_deref()
        .filter(|s| !s.trim().is_empty());
    let par_byte_count: Option<i64> =
        par_solution.map(|s| s.trim_end_matches('\n').len() as i64);
    let tournament_id = body.tournament_id.filter(|&id| id > 0);

    let result = sqlx::query(
        "INSERT INTO problems \
         (slug, title, description, difficulty, time_limit_ms, memory_limit_kb, \
          created_by, par_solution, par_byte_count, tournament_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&body.slug)
    .bind(&body.title)
    .bind(&body.description)
    .bind(&body.difficulty)
    .bind(body.time_limit_ms)
    .bind(body.memory_limit_kb)
    .bind(admin.id)
    .bind(par_solution)
    .bind(par_byte_count)
    .bind(tournament_id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            api_err(StatusCode::CONFLICT, "slug already exists")
        } else {
            tracing::error!("DB error creating problem: {e}");
            api_err(StatusCode::INTERNAL_SERVER_ERROR, "database error")
        }
    })?;

    state.cache.invalidate_problems();

    Ok((
        StatusCode::CREATED,
        Json(ProblemCreated {
            id: result.last_insert_rowid(),
            slug: body.slug,
        }),
    ))
}

// ── POST /api/admin/problems/{slug}/test-cases ────────────────────────────────

#[derive(Deserialize)]
pub struct AddTestCaseBody {
    pub input: String,
    pub expected_output: String,
    #[serde(default)]
    pub is_sample: bool,
    pub ordinal: i64,
}

#[derive(Serialize)]
pub struct TestCaseCreated {
    pub id: i64,
}

pub async fn post_api_add_test_case(
    State(state): State<AppState>,
    BearerAdmin(_admin): BearerAdmin,
    Path(slug): Path<String>,
    Json(body): Json<AddTestCaseBody>,
) -> ApiResult<TestCaseCreated> {
    let problem_row = sqlx::query("SELECT id FROM problems WHERE slug = ?")
        .bind(&slug)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| {
            tracing::error!("DB error fetching problem: {e}");
            api_err(StatusCode::INTERNAL_SERVER_ERROR, "database error")
        })?
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "problem not found"))?;

    let problem_id: i64 = problem_row.get(0);

    let result = sqlx::query(
        "INSERT INTO test_cases (problem_id, input, expected_output, is_sample, ordinal) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(problem_id)
    .bind(&body.input)
    .bind(&body.expected_output)
    .bind(body.is_sample as i64)
    .bind(body.ordinal)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("DB error adding test case: {e}");
        api_err(StatusCode::INTERNAL_SERVER_ERROR, "database error")
    })?;

    Ok((StatusCode::CREATED, Json(TestCaseCreated { id: result.last_insert_rowid() })))
}

// ── POST /api/admin/problems/{slug}/publish ───────────────────────────────────

#[derive(Serialize)]
pub struct PublishResult {
    pub slug: String,
    pub is_published: bool,
}

pub async fn post_api_toggle_publish(
    State(state): State<AppState>,
    BearerAdmin(_admin): BearerAdmin,
    Path(slug): Path<String>,
) -> ApiResult<PublishResult> {
    let row = sqlx::query("SELECT is_published FROM problems WHERE slug = ?")
        .bind(&slug)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| {
            tracing::error!("DB error: {e}");
            api_err(StatusCode::INTERNAL_SERVER_ERROR, "database error")
        })?
        .ok_or_else(|| api_err(StatusCode::NOT_FOUND, "problem not found"))?;

    let was_published: bool = row.get::<i64, _>(0) != 0;

    sqlx::query("UPDATE problems SET is_published = NOT is_published WHERE slug = ?")
        .bind(&slug)
        .execute(&state.db)
        .await
        .map_err(|e| {
            tracing::error!("DB error toggling publish: {e}");
            api_err(StatusCode::INTERNAL_SERVER_ERROR, "database error")
        })?;

    state.cache.invalidate_problems();

    Ok((
        StatusCode::OK,
        Json(PublishResult {
            slug,
            is_published: !was_published,
        }),
    ))
}

// ── GET /api/admin/tournaments ────────────────────────────────────────────────

#[derive(Serialize)]
pub struct TournamentInfo {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub is_active: bool,
}

pub async fn get_api_tournaments(
    State(state): State<AppState>,
    BearerAdmin(_admin): BearerAdmin,
) -> ApiResult<Vec<TournamentInfo>> {
    let rows = sqlx::query(
        "SELECT id, slug, name, is_active FROM tournaments \
         ORDER BY is_active DESC, created_at DESC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("DB error listing tournaments: {e}");
        api_err(StatusCode::INTERNAL_SERVER_ERROR, "database error")
    })?;

    let tournaments = rows
        .iter()
        .map(|r| TournamentInfo {
            id: r.get("id"),
            slug: r.get("slug"),
            name: r.get("name"),
            is_active: r.get::<i64, _>("is_active") != 0,
        })
        .collect();

    Ok((StatusCode::OK, Json(tournaments)))
}
