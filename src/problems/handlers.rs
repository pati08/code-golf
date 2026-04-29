use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Html,
};
use pulldown_cmark::{Event, Parser, html};
use serde::Deserialize;
use sqlx::{QueryBuilder, Row};

use crate::{app::AppState, auth::OptionalUser, error::AppError};

#[derive(Debug, Deserialize, Default, Clone)]
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
        sqlx::query("SELECT slug FROM tournaments WHERE is_active = 1 LIMIT 1")
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

    let valid_diff = params
        .difficulty
        .as_deref()
        .filter(|d| matches!(*d, "easy" | "medium" | "hard"));

    // Build query safely using query_builder to avoid format! vulnerabilities
    let mut query_builder = QueryBuilder::new(
        r#"SELECT p.id, p.slug, p.title, p.difficulty,
            CASE WHEN bs.user_id IS NOT NULL THEN 1 ELSE 0 END AS solved
        FROM problems p
        LEFT JOIN tournaments t ON t.id = p.tournament_id
        LEFT JOIN (
            SELECT DISTINCT user_id, problem_id FROM best_submissions WHERE user_id = ?
        ) bs ON bs.problem_id = p.id
        WHERE p.is_published = 1"#,
    );

    // Add tournament filter if not "all"
    if effective_tournament != "all" {
        query_builder.push(" AND t.slug = ");
        query_builder.push_bind(effective_tournament.to_string());
    }

    // Add difficulty filter if valid
    if let Some(diff) = valid_diff.clone() {
        query_builder.push(" AND p.difficulty = ");
        query_builder.push_bind(diff.to_string());
    }

    query_builder.push(" ORDER BY");
    query_builder.push(" CASE p.difficulty WHEN 'easy' THEN 1 WHEN 'medium' THEN 2 WHEN 'hard' THEN 3 ELSE 4 END,");
    query_builder.push(" p.title ASC");

    let sql = query_builder.build();

    let rows = if user_id == 0 {
        sql.fetch_all(&state.db).await?
    } else {
        let mut user_query = QueryBuilder::new(
            r#"SELECT p.id, p.slug, p.title, p.difficulty,
                CASE WHEN bs.user_id IS NOT NULL THEN 1 ELSE 0 END AS solved
            FROM problems p
            LEFT JOIN tournaments t ON t.id = p.tournament_id
            LEFT JOIN (
                SELECT DISTINCT user_id, problem_id FROM best_submissions WHERE user_id = ?
            ) bs ON bs.problem_id = p.id
            WHERE p.is_published = 1"#,
        );

        if effective_tournament != "all" {
            user_query.push(" AND t.slug = ");
            user_query.push_bind(effective_tournament.to_string());
        }

        if let Some(diff) = valid_diff {
            user_query.push(" AND p.difficulty = ");
            user_query.push_bind(diff.to_string());
        }

        user_query.push(" ORDER BY");
        user_query.push(" CASE p.difficulty WHEN 'easy' THEN 1 WHEN 'medium' THEN 2 WHEN 'hard' THEN 3 ELSE 4 END,");
        user_query.push(" p.title ASC");

        user_query.build().bind(user_id).fetch_all(&state.db).await?
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

    // Render markdown, stripping raw HTML to prevent XSS
    let parser = Parser::new(&description)
        .filter(|e| !matches!(e, Event::Html(_) | Event::InlineHtml(_)));
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
