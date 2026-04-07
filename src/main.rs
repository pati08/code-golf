mod admin;
mod app;
mod auth;
mod config;
mod db;
mod error;
mod problems;
mod runner;
mod scoreboard;
mod scoring;
mod submissions;

use std::sync::Arc;

use tower_sessions::{MemoryStore, SessionManagerLayer};
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    app::{AppState, build_templates, create_router},
    config::Config,
    runner::LanguageRegistry,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();

    // Database
    let pool = db::create_pool(&config.database_url).await?;
    db::run_migrations(&pool).await?;
    db::seed_languages(&pool).await?;

    // Session store (in-memory; fine for dev/single-node)
    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_expiry(tower_sessions::Expiry::OnInactivity(time::Duration::days(
            7,
        )));

    // Templates
    let templates = Arc::new(build_templates()?);

    // Runner
    let runner = Arc::new(LanguageRegistry::new(pool.clone()));

    let state = AppState {
        db: pool,
        templates,
        config: Arc::new(config.clone()),
        runner,
    };

    let app = create_router(state).layer(session_layer);

    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("Listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    println!("goodbye");

    Ok(())
}
