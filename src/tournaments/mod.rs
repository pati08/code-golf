use axum::{
    extract::State,
    response::Html,
};
use sqlx::Row;

use crate::{app::AppState, auth::OptionalUser, error::AppError};

pub async fn get_tournaments(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
) -> Result<Html<String>, AppError> {
    let rows = sqlx::query(
        "SELECT id, slug, name, description, is_active, start_date, end_date, created_at
         FROM tournaments ORDER BY is_active DESC, created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;

    let tournaments: Vec<_> = rows
        .iter()
        .map(|r| {
            minijinja::context! {
                slug => r.get::<String, _>("slug"),
                name => r.get::<String, _>("name"),
                description => r.get::<String, _>("description"),
                is_active => r.get::<i64, _>("is_active") != 0,
                start_date => r.get::<Option<String>, _>("start_date"),
                end_date => r.get::<Option<String>, _>("end_date"),
                created_at => r.get::<String, _>("created_at"),
            }
        })
        .collect();

    let ctx = minijinja::context! { tournaments, current_user => user };
    crate::app::render(&state.templates, "tournaments/list.html", ctx)
}

