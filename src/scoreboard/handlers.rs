use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Html,
};
use serde::Deserialize;
use sqlx::Row;

use crate::{app::AppState, auth::OptionalUser, error::AppError};

#[derive(Debug, Deserialize, Default)]
pub struct ScoreboardParams {
    #[serde(default)]
    pub tournament: Option<String>,
}

pub async fn get_global_scoreboard(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Query(params): Query<ScoreboardParams>,
    headers: HeaderMap,
) -> Result<Html<String>, AppError> {
    // Fetch tournament list for selector
    let t_rows = sqlx::query(
        "SELECT slug, name, is_active FROM tournaments ORDER BY is_active DESC, name ASC",
    )
    .fetch_all(&state.db)
    .await?;

    let all_tournaments: Vec<_> = t_rows
        .iter()
        .map(|r| {
            minijinja::context! {
                slug => r.get::<String, _>("slug"),
                name => r.get::<String, _>("name"),
                is_active => r.get::<i64, _>("is_active") != 0,
            }
        })
        .collect();

    let cookie_tournament = crate::app::get_cookie(&headers, "selectedTournament");
    let active_tournament_slug: Option<String> = sqlx::query("SELECT slug FROM tournaments ORDER BY created_at DESC LIMIT 1")
        .fetch_optional(&state.db)
        .await?
        .map(|r| r.get("slug"));

    let filter_tournament = params.tournament.as_deref().unwrap_or("");
    let effective_tournament = if !filter_tournament.is_empty() {
        filter_tournament
    } else if let Some(ref c) = cookie_tournament {
        c.as_str()
    } else {
        active_tournament_slug.as_deref().unwrap_or("all")
    };

    // Build leaderboard query
    let (entries, problems) = if effective_tournament == "all" {
        let entry_rows = sqlx::query(
            r#"SELECT u.username,
                   SUM(bs.byte_count) as total_bytes,
                   COUNT(DISTINCT bs.problem_id) as solved_count,
                   CAST(ROUND(SUM(bs.byte_count) * 1.0 / COUNT(DISTINCT bs.problem_id)) AS INTEGER) as avg_bytes,
                   SUM(CASE WHEN p.difficulty = 'easy' THEN 1 ELSE 0 END) as easy_count,
                   SUM(CASE WHEN p.difficulty = 'medium' THEN 1 ELSE 0 END) as medium_count,
                   SUM(CASE WHEN p.difficulty = 'hard' THEN 1 ELSE 0 END) as hard_count
               FROM best_submissions bs
               JOIN users u ON u.id = bs.user_id
               JOIN problems p ON p.id = bs.problem_id
               GROUP BY bs.user_id, u.username
               ORDER BY solved_count DESC, total_bytes ASC"#,
        )
        .fetch_all(&state.db)
        .await?;

        let problem_rows = sqlx::query(
            r#"SELECT p.title, p.slug, p.difficulty,
                   COUNT(bs.user_id) as solver_count,
                   MIN(bs.byte_count) as best_bytes,
                   CAST(ROUND(AVG(bs.byte_count)) AS INTEGER) as avg_bytes
               FROM problems p
               LEFT JOIN best_submissions bs ON bs.problem_id = p.id
               WHERE p.is_published = 1
               GROUP BY p.id
               ORDER BY
                   CASE p.difficulty WHEN 'easy' THEN 0 WHEN 'medium' THEN 1 WHEN 'hard' THEN 2 ELSE 3 END,
                   p.title"#,
        )
        .fetch_all(&state.db)
        .await?;

        (entry_rows, problem_rows)
    } else {
        let entry_rows = sqlx::query(
            r#"SELECT u.username,
                   SUM(bs.byte_count) as total_bytes,
                   COUNT(DISTINCT bs.problem_id) as solved_count,
                   CAST(ROUND(SUM(bs.byte_count) * 1.0 / COUNT(DISTINCT bs.problem_id)) AS INTEGER) as avg_bytes,
                   SUM(CASE WHEN p.difficulty = 'easy' THEN 1 ELSE 0 END) as easy_count,
                   SUM(CASE WHEN p.difficulty = 'medium' THEN 1 ELSE 0 END) as medium_count,
                   SUM(CASE WHEN p.difficulty = 'hard' THEN 1 ELSE 0 END) as hard_count
               FROM best_submissions bs
               JOIN users u ON u.id = bs.user_id
               JOIN problems p ON p.id = bs.problem_id
               JOIN tournaments t ON t.id = p.tournament_id
               WHERE t.slug = ?
               GROUP BY bs.user_id, u.username
               ORDER BY solved_count DESC, total_bytes ASC"#,
        )
        .bind(effective_tournament)
        .fetch_all(&state.db)
        .await?;

        let problem_rows = sqlx::query(
            r#"SELECT p.title, p.slug, p.difficulty,
                   COUNT(bs.user_id) as solver_count,
                   MIN(bs.byte_count) as best_bytes,
                   CAST(ROUND(AVG(bs.byte_count)) AS INTEGER) as avg_bytes
               FROM problems p
               LEFT JOIN best_submissions bs ON bs.problem_id = p.id
               JOIN tournaments t ON t.id = p.tournament_id
               WHERE p.is_published = 1 AND t.slug = ?
               GROUP BY p.id
               ORDER BY
                   CASE p.difficulty WHEN 'easy' THEN 0 WHEN 'medium' THEN 1 WHEN 'hard' THEN 2 ELSE 3 END,
                   p.title"#,
        )
        .bind(effective_tournament)
        .fetch_all(&state.db)
        .await?;

        (entry_rows, problem_rows)
    };

    let entries: Vec<_> = entries
        .iter()
        .map(|r| {
            minijinja::context! {
                username => r.get::<String, _>("username"),
                total_bytes => r.get::<i64, _>("total_bytes"),
                solved_count => r.get::<i64, _>("solved_count"),
                avg_bytes => r.get::<i64, _>("avg_bytes"),
                easy_count => r.get::<i64, _>("easy_count"),
                medium_count => r.get::<i64, _>("medium_count"),
                hard_count => r.get::<i64, _>("hard_count"),
            }
        })
        .collect();

    let problems: Vec<_> = problems
        .iter()
        .map(|r| {
            minijinja::context! {
                title => r.get::<String, _>("title"),
                slug => r.get::<String, _>("slug"),
                difficulty => r.get::<String, _>("difficulty"),
                solver_count => r.get::<i64, _>("solver_count"),
                best_bytes => r.get::<Option<i64>, _>("best_bytes"),
                avg_bytes => r.get::<Option<i64>, _>("avg_bytes"),
            }
        })
        .collect();

    let total_problems = problems.len();

    let ctx = minijinja::context! {
        entries,
        problems,
        total_problems,
        all_tournaments,
        filter_tournament => effective_tournament,
        active_tournament_slug => active_tournament_slug.as_deref().unwrap_or(""),
        current_user => user,
    };
    crate::app::render(&state.templates, "scoreboard/global.html", ctx)
}

pub async fn get_problem_scoreboard(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    OptionalUser(user): OptionalUser,
) -> Result<Html<String>, AppError> {
    let problem_row = sqlx::query(
        "SELECT id, title FROM problems WHERE slug = ? AND is_published = 1",
    )
    .bind(&slug)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let problem_id: i64 = problem_row.get("id");
    let problem_title: String = problem_row.get("title");

    let rows = sqlx::query(
        r#"SELECT u.username, l.display_name as language_name, bs.byte_count,
               s.created_at as submitted_at
           FROM best_submissions bs
           JOIN users u ON u.id = bs.user_id
           JOIN languages l ON l.id = bs.language_id
           JOIN submissions s ON s.id = bs.submission_id
           WHERE bs.problem_id = ?
           ORDER BY bs.byte_count ASC, s.created_at ASC"#,
    )
    .bind(problem_id)
    .fetch_all(&state.db)
    .await?;

    let entries: Vec<_> = rows
        .iter()
        .map(|r| {
            minijinja::context! {
                username => r.get::<String, _>("username"),
                language_name => r.get::<String, _>("language_name"),
                byte_count => r.get::<i64, _>("byte_count"),
                submitted_at => r.get::<String, _>("submitted_at"),
            }
        })
        .collect();

    let ctx = minijinja::context! {
        problem => minijinja::context! { slug, title => problem_title },
        entries,
        current_user => user,
    };
    crate::app::render(&state.templates, "scoreboard/problem.html", ctx)
}
