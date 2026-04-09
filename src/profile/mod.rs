use axum::{extract::State, response::Html};
use sqlx::Row;

use crate::{app::AppState, auth::RequiredUser, error::AppError};

pub async fn get_profile(
    State(state): State<AppState>,
    RequiredUser(user): RequiredUser,
) -> Result<Html<String>, AppError> {
    // Member since date
    let user_row = sqlx::query("SELECT created_at FROM users WHERE id = ?")
        .bind(user.id)
        .fetch_one(&state.db)
        .await?;
    let created_at: String = user_row.get("created_at");
    let member_since = created_at.get(..10).unwrap_or(&created_at).to_string();

    // Overall stats
    let stats_row = sqlx::query(
        "SELECT COUNT(DISTINCT problem_id) as solved_count, \
         COALESCE(SUM(byte_count), 0) as total_bytes \
         FROM best_submissions WHERE user_id = ?",
    )
    .bind(user.id)
    .fetch_one(&state.db)
    .await?;
    let solved_count: i64 = stats_row.get("solved_count");
    let total_bytes: i64 = stats_row.get("total_bytes");

    // Global rank: how many users beat this user (more solves, or equal solves with fewer bytes)
    let rank: Option<i64> = if solved_count > 0 {
        let rank_row = sqlx::query(
            r#"SELECT COUNT(*) + 1 as rank FROM (
                 SELECT user_id,
                   COUNT(DISTINCT problem_id) as solved,
                   SUM(byte_count) as total_bytes
                 FROM best_submissions GROUP BY user_id
               ) ranked
               WHERE solved > ? OR (solved = ? AND total_bytes < ?)"#,
        )
        .bind(solved_count)
        .bind(solved_count)
        .bind(total_bytes)
        .fetch_one(&state.db)
        .await?;
        Some(rank_row.get("rank"))
    } else {
        None
    };

    // Per-problem breakdown sorted by: solved first, then gap desc, then title
    let problem_rows = sqlx::query(
        r#"WITH user_best AS (
             SELECT problem_id, MIN(byte_count) AS min_bytes
             FROM best_submissions
             WHERE user_id = ?
             GROUP BY problem_id
           ),
           global_best AS (
             SELECT problem_id, MIN(byte_count) AS min_bytes
             FROM best_submissions
             GROUP BY problem_id
           ),
           user_rank AS (
             SELECT bs.problem_id, COUNT(DISTINCT bs.user_id) + 1 AS rank
             FROM best_submissions bs
             JOIN user_best ub ON ub.problem_id = bs.problem_id
             WHERE (
               SELECT MIN(byte_count) FROM best_submissions
               WHERE problem_id = bs.problem_id AND user_id = bs.user_id
             ) < ub.min_bytes
             GROUP BY bs.problem_id
           ),
           solver_count AS (
             SELECT problem_id, COUNT(DISTINCT user_id) AS total
             FROM best_submissions
             GROUP BY problem_id
           )
           SELECT
             p.slug, p.title, p.difficulty,
             ub.min_bytes        AS user_best,
             gb.min_bytes        AS global_best,
             COALESCE(ur.rank, 1) AS user_rank,
             COALESCE(sc.total, 0) AS total_solvers
           FROM problems p
           LEFT JOIN user_best ub ON ub.problem_id = p.id
           LEFT JOIN global_best gb ON gb.problem_id = p.id
           LEFT JOIN user_rank ur ON ur.problem_id = p.id
           LEFT JOIN solver_count sc ON sc.problem_id = p.id
           WHERE p.is_published = 1
           ORDER BY
             CASE WHEN ub.min_bytes IS NOT NULL THEN 0 ELSE 1 END,
             (CAST(ub.min_bytes AS REAL) - CAST(gb.min_bytes AS REAL)) DESC,
             p.title ASC"#,
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;

    let mut solved_problems = Vec::new();
    let mut unsolved_problems = Vec::new();

    for r in &problem_rows {
        let user_best: Option<i64> = r.get("user_best");
        let global_best: Option<i64> = r.get("global_best");
        let gap = user_best.zip(global_best).map(|(u, g)| u - g);
        let gap_pct = user_best.zip(global_best).map(|(u, g)| {
            if g == 0 {
                0.0_f64
            } else {
                (u - g) as f64 / g as f64 * 100.0
            }
        });
        let user_rank: Option<i64> = if user_best.is_some() {
            Some(r.get("user_rank"))
        } else {
            None
        };
        let total_solvers: i64 = r.get("total_solvers");

        let entry = minijinja::context! {
            slug => r.get::<String, _>("slug"),
            title => r.get::<String, _>("title"),
            difficulty => r.get::<String, _>("difficulty"),
            user_best,
            global_best,
            gap,
            gap_pct,
            user_rank,
            total_solvers,
        };

        if user_best.is_some() {
            solved_problems.push(entry);
        } else {
            unsolved_problems.push(entry);
        }
    }

    let current_user_ctx = minijinja::context! {
        id => user.id,
        username => user.username.clone(),
        is_admin => user.is_admin,
    };

    let ctx = minijinja::context! {
        current_user => current_user_ctx,
        member_since,
        stats => minijinja::context! {
            solved_count,
            total_bytes,
            rank,
        },
        solved_problems,
        unsolved_problems,
    };

    crate::app::render(&state.templates, "profile/index.html", ctx)
}
