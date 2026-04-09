use axum::{
    extract::{Path, State},
    response::Html,
};
use sqlx::Row;

use crate::{app::AppState, auth::OptionalUser, error::AppError};

pub async fn get_global_scoreboard(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
) -> Result<Html<String>, AppError> {
    let rows = sqlx::query(
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

    let entries: Vec<_> = rows
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

    let problems: Vec<_> = problem_rows
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

    let ctx = minijinja::context! { entries, problems, total_problems, current_user => user };
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
