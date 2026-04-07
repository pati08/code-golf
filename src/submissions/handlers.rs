use axum::{
    Form,
    extract::{Path, State},
    response::Html,
};
use serde::Deserialize;
use sqlx::Row;

use crate::{
    app::AppState,
    auth::{OptionalUser, RequiredUser},
    error::AppError,
    submissions::judge,
};

#[derive(Deserialize)]
pub struct SubmitForm {
    pub language_id: i64,
    pub code: String,
}

pub async fn post_submit(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    RequiredUser(user): RequiredUser,
    Form(form): Form<SubmitForm>,
) -> Result<Html<String>, AppError> {
    if form.code.len() > 65536 {
        return Err(AppError::BadRequest("Code exceeds 64 KB limit".to_string()));
    }

    let problem_row = sqlx::query(
        "SELECT id FROM problems WHERE slug = ? AND is_published = 1",
    )
    .bind(&slug)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let problem_id: i64 = problem_row.get("id");

    let lang = state
        .runner
        .get_by_id(form.language_id)
        .await?
        .filter(|l| l.is_enabled)
        .ok_or_else(|| AppError::BadRequest("Invalid or disabled language".to_string()))?;

    let byte_count = form.code.trim_end_matches('\n').len() as i64;

    let result = sqlx::query(
        "INSERT INTO submissions (user_id, problem_id, language_id, code, byte_count) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(user.id)
    .bind(problem_id)
    .bind(lang.id)
    .bind(&form.code)
    .bind(byte_count)
    .execute(&state.db)
    .await?;

    let submission_id = result.last_insert_rowid();

    let pool = state.db.clone();
    let runner = state.runner.clone();
    tokio::spawn(judge::run(submission_id, pool, runner));

    let html = format!(
        "<div hx-get=\"/submissions/{submission_id}\" hx-trigger=\"every 1s\" hx-swap=\"innerHTML\" hx-target=\"#submission-result\" class=\"submission-pending\">\
            <p>Judging submission #{submission_id}...</p>\
        </div>"
    );
    Ok(Html(html))
}

async fn fetch_submission_ctx(
    state: &AppState,
    id: i64,
) -> Result<minijinja::value::Value, AppError> {
    let row = sqlx::query(
        r#"SELECT s.id, s.status, s.byte_count, s.error_output, s.created_at, s.judged_at,
               u.username, p.title as problem_title, p.slug as problem_slug,
               l.display_name as language_name, p.par_byte_count
           FROM submissions s
           JOIN users u ON u.id = s.user_id
           JOIN problems p ON p.id = s.problem_id
           JOIN languages l ON l.id = s.language_id
           WHERE s.id = ?"#,
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let status = row.get::<String, _>("status");
    let byte_count = row.get::<i64, _>("byte_count");
    let par_byte_count = row.get::<Option<i64>, _>("par_byte_count");

    let par_score: Option<i32> = if status == "accepted" {
        par_byte_count.map(|p| crate::scoring::compute_par_score(byte_count, p))
    } else {
        None
    };
    let par_score_name: Option<&str> = par_score.map(crate::scoring::par_score_name);

    Ok(minijinja::context! {
        id => row.get::<i64, _>("id"),
        status => &status,
        byte_count => byte_count,
        error_output => row.get::<Option<String>, _>("error_output"),
        created_at => row.get::<String, _>("created_at"),
        judged_at => row.get::<Option<String>, _>("judged_at"),
        username => row.get::<String, _>("username"),
        problem_title => row.get::<String, _>("problem_title"),
        problem_slug => row.get::<String, _>("problem_slug"),
        language_name => row.get::<String, _>("language_name"),
        par_score => par_score,
        par_score_name => par_score_name,
    })
}

/// HTMX polling endpoint — returns a fragment
pub async fn get_submission(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    OptionalUser(user): OptionalUser,
) -> Result<Html<String>, AppError> {
    let sub = fetch_submission_ctx(&state, id).await?;
    let status = sub.get_attr("status").unwrap().to_string();

    if status == "pending" || status == "running" {
        let html = format!(
            "<div hx-get=\"/submissions/{id}\" hx-trigger=\"every 1s\" hx-swap=\"innerHTML\" hx-target=\"#submission-result\" class=\"submission-pending\">\
                <p>Judging submission #{id}...</p>\
            </div>"
        );
        return Ok(Html(html));
    }

    let ctx = minijinja::context! {
        submission => sub,
        current_user => user,
    };
    crate::app::render(&state.templates, "submissions/detail.html", ctx)
}

/// Full page view
pub async fn get_submission_page(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    OptionalUser(user): OptionalUser,
) -> Result<Html<String>, AppError> {
    let sub = fetch_submission_ctx(&state, id).await?;
    let ctx = minijinja::context! {
        submission => sub,
        current_user => user,
    };
    crate::app::render(&state.templates, "submissions/result.html", ctx)
}
