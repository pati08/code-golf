use std::sync::{Arc, RwLock};

use axum::{
    Router,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    http::{StatusCode, HeaderMap},
};
use minijinja::Environment;
use sqlx::SqlitePool;
use tower_http::{compression::CompressionLayer, services::ServeDir, trace::TraceLayer};

use crate::{
    admin::{api as admin_api, handlers as admin}, auth::handlers as auth, cache::AppCache, config::Config,
    error::AppError, feedback::handlers as feedback, problems::handlers as problems, profile,
    rate_limit::RateLimiters, runner::LanguageRegistry, scoreboard::handlers as scoreboard,
    submissions::handlers as submissions, tournaments,
};

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub templates: Arc<RwLock<Environment<'static>>>,
    pub config: Arc<Config>,
    pub runner: Arc<LanguageRegistry>,
    pub cache: AppCache,
    pub rate_limiters: RateLimiters,
}

pub fn render(
    env: &RwLock<Environment<'static>>,
    template: &str,
    ctx: impl serde::Serialize,
) -> Result<Html<String>, AppError> {
    let env = env.read().expect("poisoned lock");
    let tmpl = env.get_template(template)?;
    let rendered = tmpl.render(ctx)?;
    Ok(Html(rendered))
}

pub fn get_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .filter_map(|s| s.trim().split_once('='))
                .find(|(k, _)| k.trim() == name)
                .map(|(_, v)| v.trim().to_owned())
        })
}

async fn handle_404() -> Response {
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>404 - Code Golf</title>
  <link rel="stylesheet" href="/static/style.css">
</head>
<body>
  <nav class="navbar">
    <div class="nav-brand"><a href="/">⛳ Code Golf</a></div>
  </nav>

  <main class="container">
    <div class="error-container">
      <div class="error-code">404</div>
      <h1 class="error-title">Not Found</h1>
      <p class="error-message">The page you're looking for doesn't exist.</p>
      <p class="error-suggestion">Check the URL and try again, or return to the home page.</p>
      <a href="/" class="btn btn-primary">Go Home</a>
    </div>
  </main>

  <footer class="site-footer">
    <p>Code Golf Platform</p>
  </footer>
</body>
</html>"#;
    (StatusCode::NOT_FOUND, Html(html)).into_response()
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Public
        .route("/", get(problems::get_index))
        .route("/profile", get(profile::get_profile))
        .route("/problems", get(problems::get_problems))
        .route("/problems/{slug}", get(problems::get_problem))
        .route("/problems/{slug}/submit", post(submissions::post_submit))
        .route("/submissions/{id}", get(submissions::get_submission))
        .route(
            "/submissions/{id}/view",
            get(submissions::get_submission_page),
        )
        .route("/scoreboard", get(scoreboard::get_global_scoreboard))
        .route(
            "/problems/{slug}/scoreboard",
            get(scoreboard::get_problem_scoreboard),
        )
        .route("/tournaments", get(tournaments::get_tournaments))
        // Feedback
        .route("/feedback/form", get(feedback::get_feedback_form))
        .route("/feedback", post(feedback::post_feedback))
        // Auth
        .route(
            "/register",
            get(auth::get_register).post(auth::post_register),
        )
        .route("/login", get(auth::get_login).post(auth::post_login))
        .route("/logout", post(auth::post_logout))
        // Admin
        .route("/admin", get(admin::get_dashboard))
        .route(
            "/admin/problems",
            get(admin::get_admin_problems).post(admin::post_create_problem),
        )
        .route("/admin/problems/new", get(admin::get_new_problem))
        .route("/admin/problems/{slug}/edit", get(admin::get_edit_problem))
        .route("/admin/problems/{slug}", post(admin::post_update_problem))
        .route(
            "/admin/problems/{slug}/publish",
            post(admin::post_toggle_publish),
        )
        .route(
            "/admin/problems/{slug}/test-cases",
            get(admin::get_test_cases).post(admin::post_add_test_case),
        )
        .route(
            "/admin/test-cases/{id}/delete",
            post(admin::post_delete_test_case),
        )
        .route("/admin/submissions", get(admin::get_admin_submissions))
        .route("/admin/users", get(admin::get_admin_users))
        .route(
            "/admin/users/{id}/toggle-admin",
            post(admin::post_toggle_admin),
        )
        .route("/admin/feedback", get(admin::get_admin_feedback))
        .route(
            "/admin/feedback/{id}/status",
            post(admin::post_feedback_status),
        )
        // Admin API keys
        .route(
            "/admin/api-keys",
            get(admin::get_admin_api_keys).post(admin::post_create_api_key),
        )
        .route("/admin/api-keys/{id}/revoke", post(admin::post_revoke_api_key))
        .route("/admin/api-keys/{id}/delete", post(admin::post_delete_api_key))
        // Admin JSON API (bearer token auth)
        .route("/api/admin/tournaments", get(admin_api::get_api_tournaments))
        .route(
            "/api/admin/problems",
            post(admin_api::post_api_create_problem),
        )
        .route(
            "/api/admin/problems/{slug}/test-cases",
            post(admin_api::post_api_add_test_case),
        )
        .route(
            "/api/admin/problems/{slug}/publish",
            post(admin_api::post_api_toggle_publish),
        )
        .route(
            "/admin/tournaments",
            get(admin::get_admin_tournaments).post(admin::post_create_tournament),
        )
        .route("/admin/tournaments/new", get(admin::get_new_tournament))
        .route(
            "/admin/tournaments/{slug}/edit",
            get(admin::get_edit_tournament),
        )
        .route(
            "/admin/tournaments/{slug}",
            post(admin::post_update_tournament),
        )
        .route(
            "/admin/tournaments/{slug}/set-active",
            post(admin::post_set_active_tournament),
        )
        // Static files
        .nest_service("/static", ServeDir::new("static"))
        .fallback(handle_404)
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .with_state(state)
}

pub fn build_templates() -> anyhow::Result<Environment<'static>> {
    let mut env = Environment::new();
    env.set_loader(minijinja::path_loader("templates"));
    Ok(env)
}
