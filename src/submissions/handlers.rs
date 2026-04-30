use axum::{
    Form,
    extract::{Path, State},
    response::Html,
};
use serde::Deserialize;
use sqlx::Row;
use tower_sessions::Session;

use crate::{app::AppState, auth::{OptionalUser, RequiredUser}, error::AppError, submissions::judge};

#[derive(Deserialize)]
pub struct SubmitForm {
    pub language_id: i64,
    pub code: String,
    pub csrf_token: String,
}

pub async fn post_submit(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    RequiredUser(user): RequiredUser,
    session: Session,
    Form(form): Form<SubmitForm>,
) -> Result<Html<String>, AppError> {
    crate::csrf::validate(&session, &form.csrf_token).await?;

    if !state.rate_limiters.submit.check(user.id.to_string()).await {
        return Err(AppError::BadRequest("Submission rate limit exceeded. Try again shortly.".to_string()));
    }

    if form.code.len() > state.config.max_code_size {
        return Err(AppError::BadRequest("Code exceeds limit".to_string()));
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

    let code = form.code.replace("\r\n", "\n");
    let byte_count = code.trim_end_matches('\n').len() as i64;

    let result = sqlx::query(
        "INSERT INTO submissions (user_id, problem_id, language_id, code, byte_count) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(user.id)
    .bind(problem_id)
    .bind(lang.id)
    .bind(&code)
    .bind(byte_count)
    .execute(&state.db)
    .await?;

    let submission_id = result.last_insert_rowid();

    let pool = state.db.clone();
    let runner = state.runner.clone();
    let semaphore = state.judge_semaphore.clone();
    tokio::spawn(async move {
        let _permit = semaphore.acquire_owned().await.expect("judge semaphore closed");
        judge::run(submission_id, pool, runner).await;
    });

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
        r#"SELECT s.id, s.status, s.byte_count, s.error_output, s.formatted_code, s.formatted_error_output, s.created_at, s.judged_at,
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
    let error_output = row.get::<Option<String>, _>("error_output");

    let par_score: Option<i32> = if status == "accepted" {
        par_byte_count.map(|p| crate::scoring::compute_par_score(byte_count, p))
    } else {
        None
    };
    let par_score_name: Option<&str> = par_score.map(crate::scoring::par_score_name);

    // For wrong_answer, error_output may contain JSON with sample test case details.
    // Parse it into structured fields and don't expose the raw JSON to templates.
    let (wa_input, wa_expected, wa_actual, display_error_output) =
        if status == "wrong_answer" {
            if let Some(ref s) = error_output {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
                    (
                        v["input"].as_str().map(str::to_string),
                        v["expected"].as_str().map(str::to_string),
                        v["actual"].as_str().map(str::to_string),
                        None::<String>,
                    )
                } else {
                    (None, None, None, error_output.clone())
                }
            } else {
                (None, None, None, None)
            }
        } else {
            (None, None, None, error_output)
        };

    Ok(minijinja::context! {
        id => row.get::<i64, _>("id"),
        status => &status,
        byte_count => byte_count,
        error_output => display_error_output,
        formatted_code => row.get::<Option<String>, _>("formatted_code"),
        formatted_error_output => row.get::<Option<String>, _>("formatted_error_output"),
        created_at => row.get::<String, _>("created_at"),
        judged_at => row.get::<Option<String>, _>("judged_at"),
        username => row.get::<String, _>("username"),
        problem_title => row.get::<String, _>("problem_title"),
        problem_slug => row.get::<String, _>("problem_slug"),
        language_name => row.get::<String, _>("language_name"),
        par_score => par_score,
        par_score_name => par_score_name,
        wrong_answer_input => wa_input,
        wrong_answer_expected => wa_expected,
        wrong_answer_actual => wa_actual,
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
