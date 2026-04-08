use std::sync::{Arc, RwLock};

use axum::{
    Router,
    response::Html,
    routing::{get, post},
};
use minijinja::Environment;
use sqlx::SqlitePool;
use tower_http::{compression::CompressionLayer, services::ServeDir, trace::TraceLayer};

use crate::{
    admin::handlers as admin, auth::handlers as auth, config::Config, error::AppError,
    problems::handlers as problems, runner::LanguageRegistry, scoreboard::handlers as scoreboard,
    submissions::handlers as submissions,
};

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub templates: Arc<RwLock<Environment<'static>>>,
    #[allow(dead_code)]
    pub config: Arc<Config>,
    pub runner: Arc<LanguageRegistry>,
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

pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Public
        .route("/", get(problems::get_index))
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
        // Static files
        .nest_service("/static", ServeDir::new("static"))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .with_state(state)
}

pub fn build_templates() -> anyhow::Result<Environment<'static>> {
    let mut env = Environment::new();
    env.set_loader(minijinja::path_loader("templates"));
    Ok(env)
}
