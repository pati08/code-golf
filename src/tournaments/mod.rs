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
    // Try full-list cache first (JSON-serialized context values)
    if let Some(cached) = state.cache.tournament.get_full_list().await {
        let tournaments: Vec<minijinja::Value> = serde_json::from_str(&cached)
            .unwrap_or_default();
        let ctx = minijinja::context! { tournaments, current_user => user };
        return crate::app::render(&state.templates, "tournaments/list.html", ctx);
    }

    let rows = sqlx::query(
        "SELECT id, slug, name, description, is_active, start_date, end_date, created_at
         FROM tournaments ORDER BY is_active DESC, created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;

    let tournaments: Vec<minijinja::Value> = rows
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

    // Serialize and store in cache
    let tournaments_json = serde_json::to_value(&tournaments)
        .ok()
        .map(|v| v.to_string())
        .unwrap_or_default();
    if !tournaments_json.is_empty() {
        state.cache.tournament.set_full_list(tournaments_json).await;
    }

    // Also seed the lightweight list cache
    let lightweight: crate::cache::TournamentList = rows
        .iter()
        .map(|r| {
            (
                r.get::<String, _>("slug"),
                r.get::<String, _>("name"),
                r.get::<i64, _>("is_active") != 0,
            )
        })
        .collect();
    state.cache.tournament.set_list(lightweight).await;

    let ctx = minijinja::context! { tournaments, current_user => user };
    crate::app::render(&state.templates, "tournaments/list.html", ctx)
}
