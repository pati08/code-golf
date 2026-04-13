use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Html,
};
use pulldown_cmark::{Parser, html};
use serde::Deserialize;
use sqlx::Row;

use crate::{app::AppState, auth::OptionalUser, error::AppError};

#[derive(Debug, Deserialize, Default)]
pub struct FilterParams {
    #[serde(default)]
    pub difficulty: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub tournament: Option<String>,
}

async fn fetch_tournament_list(
    db: &sqlx::SqlitePool,
) -> Result<Vec<minijinja::Value>, AppError> {
    let rows = sqlx::query(
        "SELECT slug, name, is_active FROM tournaments ORDER BY is_active DESC, name ASC",
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .iter()
        .map(|r| {
            minijinja::context! {
                slug => r.get::<String, _>("slug"),
                name => r.get::<String, _>("name"),
                is_active => r.get::<i64, _>("is_active") != 0,
            }
        })
        .collect())
}

async fn fetch_active_tournament_slug(db: &sqlx::SqlitePool) -> Result<Option<String>, AppError> {
    Ok(
        sqlx::query("SELECT slug FROM tournaments ORDER BY created_at DESC LIMIT 1")
            .fetch_optional(db)
            .await?
            .map(|r| r.get(0)),
    )
}

pub async fn get_problems(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Query(params): Query<FilterParams>,
    headers: HeaderMap,
) -> Result<Html<String>, AppError> {
    let user_id: i64 = user.as_ref().map(|u| u.id).unwrap_or(0);

    let cookie_tournament = crate::app::get_cookie(&headers, "selectedTournament");
    let active_tournament_slug = fetch_active_tournament_slug(&state.db).await?;
    let all_tournaments = fetch_tournament_list(&state.db).await?;

    // Determine effective tournament filter: query param > cookie > default
    let filter_tournament = params.tournament.as_deref().unwrap_or("");
    let effective_tournament = if !filter_tournament.is_empty() {
        filter_tournament
    } else if let Some(ref c) = cookie_tournament {
        c.as_str()
    } else {
        active_tournament_slug.as_deref().unwrap_or("all")
    };

    let diff_clause = match params.difficulty.as_deref() {
        Some("easy") | Some("medium") | Some("hard") => "AND p.difficulty = ?",
        _ => "",
    };

    let tournament_clause = if effective_tournament == "all" {
        ""
    } else {
        "AND t.slug = ?"
    };

    let sql = format!(
        r#"SELECT
            p.id, p.slug, p.title, p.difficulty,
            CASE WHEN bs.user_id IS NOT NULL THEN 1 ELSE 0 END AS solved
        FROM problems p
        LEFT JOIN tournaments t ON t.id = p.tournament_id
        LEFT JOIN (
            SELECT DISTINCT user_id, problem_id FROM best_submissions WHERE user_id = ?
        ) bs ON bs.problem_id = p.id
        WHERE p.is_published = 1 {tournament_clause} {diff_clause}
        ORDER BY
            CASE p.difficulty WHEN 'easy' THEN 1 WHEN 'medium' THEN 2 WHEN 'hard' THEN 3 ELSE 4 END,
            p.title ASC"#
    );

    let valid_diff = params
        .difficulty
        .as_deref()
        .filter(|d| matches!(*d, "easy" | "medium" | "hard"));

    let rows = match (effective_tournament == "all", valid_diff) {
        (true, None) => sqlx::query(&sql).bind(user_id).fetch_all(&state.db).await?,
        (true, Some(diff)) => {
            sqlx::query(&sql)
                .bind(user_id)
                .bind(diff)
                .fetch_all(&state.db)
                .await?
        }
        (false, None) => {
            sqlx::query(&sql)
                .bind(user_id)
                .bind(effective_tournament)
                .fetch_all(&state.db)
                .await?
        }
        (false, Some(diff)) => {
            sqlx::query(&sql)
                .bind(user_id)
                .bind(effective_tournament)
                .bind(diff)
                .fetch_all(&state.db)
                .await?
        }
    };

    // Build problem list with solved flag
    let all_items: Vec<(minijinja::Value, bool)> = rows
        .iter()
        .map(|r| {
            let solved = r.get::<i64, _>("solved") != 0;
            let ctx = minijinja::context! {
                slug => r.get::<String, _>("slug"),
                title => r.get::<String, _>("title"),
                difficulty => r.get::<String, _>("difficulty"),
            };
            (ctx, solved)
        })
        .collect();

    let is_logged_in = user.is_some();

    let show_solved = !matches!(params.status.as_deref(), Some("unsolved"));
    let show_unsolved = !matches!(params.status.as_deref(), Some("solved"));

    let solved_problems: Vec<_> = all_items
        .iter()
        .filter(|(_, s)| *s && show_solved && is_logged_in)
        .map(|(ctx, _)| ctx.clone())
        .collect();

    let unsolved_problems: Vec<_> = all_items
        .iter()
        .filter(|(_, s)| !*s && show_unsolved && is_logged_in)
        .map(|(ctx, _)| ctx.clone())
        .collect();

    let all_problems: Vec<_> = if !is_logged_in {
        all_items.into_iter().map(|(ctx, _)| ctx).collect()
    } else {
        vec![]
    };

    let ctx = minijinja::context! {
        all_problems,
        solved_problems,
        unsolved_problems,
        current_user => user,
        filter_difficulty => params.difficulty.as_deref().unwrap_or(""),
        filter_status => params.status.as_deref().unwrap_or(""),
        filter_tournament => effective_tournament,
        active_tournament_slug => active_tournament_slug.as_deref().unwrap_or(""),
        all_tournaments,
        is_logged_in,
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
