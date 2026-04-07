use axum::{
    extract::{Path, State},
    response::Html,
};
use pulldown_cmark::{Parser, html};
use sqlx::Row;

use crate::{app::AppState, auth::OptionalUser, error::AppError};

pub async fn get_problems(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
) -> Result<Html<String>, AppError> {
    let rows = sqlx::query(
        "SELECT id, slug, title, difficulty, created_at FROM problems WHERE is_published = 1 ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;

    let problems: Vec<_> = rows
        .iter()
        .map(|r| {
            minijinja::context! {
                id => r.get::<i64, _>("id"),
                slug => r.get::<String, _>("slug"),
                title => r.get::<String, _>("title"),
                difficulty => r.get::<String, _>("difficulty"),
                created_at => r.get::<String, _>("created_at"),
            }
        })
        .collect();

    let ctx = minijinja::context! {
        problems,
        current_user => user,
    };
    crate::app::render(&state.templates, "problems/list.html", ctx)
}

pub async fn get_problem(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    OptionalUser(user): OptionalUser,
) -> Result<Html<String>, AppError> {
    let row = sqlx::query(
        "SELECT id, slug, title, description, difficulty, time_limit_ms, memory_limit_kb FROM problems WHERE slug = ? AND is_published = 1",
    )
    .bind(&slug)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let problem_id: i64 = row.get("id");
    let description: String = row.get("description");

    // Render markdown
    let parser = Parser::new(&description);
    let mut description_html = String::new();
    html::push_html(&mut description_html, parser);

    // Sample test cases
    let sample_rows = sqlx::query(
        "SELECT input, expected_output FROM test_cases WHERE problem_id = ? AND is_sample = 1 ORDER BY ordinal",
    )
    .bind(problem_id)
    .fetch_all(&state.db)
    .await?;

    let samples: Vec<_> = sample_rows
        .iter()
        .map(|r| {
            minijinja::context! {
                input => r.get::<String, _>("input"),
                expected_output => r.get::<String, _>("expected_output"),
            }
        })
        .collect();

    let languages = state.runner.get_enabled().await?;

    let ctx = minijinja::context! {
        problem => minijinja::context! {
            id => problem_id,
            slug => row.get::<String, _>("slug"),
            title => row.get::<String, _>("title"),
            description_html,
            difficulty => row.get::<String, _>("difficulty"),
            time_limit_ms => row.get::<i64, _>("time_limit_ms"),
            memory_limit_kb => row.get::<i64, _>("memory_limit_kb"),
        },
        samples,
        languages,
        current_user => user,
    };
    crate::app::render(&state.templates, "problems/detail.html", ctx)
}

pub async fn get_index(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
) -> Result<Html<String>, AppError> {
    let rows = sqlx::query(
        "SELECT slug, title, difficulty FROM problems WHERE is_published = 1 ORDER BY created_at DESC LIMIT 6",
    )
    .fetch_all(&state.db)
    .await?;

    let featured: Vec<_> = rows
        .iter()
        .map(|r| {
            minijinja::context! {
                slug => r.get::<String, _>("slug"),
                title => r.get::<String, _>("title"),
                difficulty => r.get::<String, _>("difficulty"),
            }
        })
        .collect();

    let ctx = minijinja::context! {
        featured,
        current_user => user,
    };
    crate::app::render(&state.templates, "index.html", ctx)
}
