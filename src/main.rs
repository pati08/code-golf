mod admin;
mod app;
mod auth;
mod config;
mod db;
mod error;
mod feedback;
mod problems;
mod profile;
mod runner;
mod scoreboard;
mod scoring;
mod submissions;
mod tournaments;

use std::sync::{Arc, RwLock};

use axum::{
    body::Body,
    response::{IntoResponse, Response},
};
use hyper::{Method, Request, StatusCode, body::Incoming, server::conn::http1, upgrade::Upgraded};
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use tower::{Service, ServiceExt};
use tower_sessions::SessionManagerLayer;
use tower_sessions_sqlx_store::SqliteStore;
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

    // Session store
    let session_store = SqliteStore::new(pool.clone());
    session_store.migrate().await?;
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_expiry(tower_sessions::Expiry::OnInactivity(time::Duration::days(
            7,
        )));

    // Templates
    let templates = Arc::new(RwLock::new(build_templates()?));

    // Runner
    let runner = Arc::new(LanguageRegistry::new(pool.clone()));

    // Spawn task to refresh templates
    #[cfg(debug_assertions)]
    tokio::task::spawn_blocking({
        let templates = Arc::clone(&templates);
        move || {
            use std::{path::Path, sync::mpsc};

            use notify::Watcher;

            let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();

            let mut watcher = match notify::recommended_watcher(tx) {
                Ok(w) => w,
                Err(e) => {
                    tracing::error!(
                        "Failed to get recommended watcher for templates directory: {e}"
                    );
                    return;
                }
            };

            if let Err(e) =
                watcher.watch(Path::new("./templates"), notify::RecursiveMode::Recursive)
            {
                tracing::error!("Failed to watch templates directory: {e}");
                return;
            }

            for res in rx {
                match res {
                    Ok(event)
                        if event.kind.is_modify()
                            || event.kind.is_remove()
                            || event.kind.is_create() =>
                    {
                        tracing::info!("Reloading templates...");
                        let new_templates = match build_templates() {
                            Ok(t) => t,
                            Err(e) => {
                                tracing::error!("Error building templates: {e}");
                                continue;
                            }
                        };
                        let mut lock = templates.write().expect("Poisoned lock");
                        *lock = new_templates;
                    }
                    Ok(_) => (),
                    Err(e) => tracing::error!("Error watching templates: {e}"),
                }
            }
        }
    });

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

    let tower_service = tower::service_fn(move |req: Request<_>| {
        let router_svc = app.clone();
        let req = req.map(Body::new);
        async move {
            if req.method() == Method::CONNECT {
                proxy(req).await
            } else {
                router_svc.oneshot(req).await.map_err(|err| match err {})
            }
        }
    });

    let hyper_service =
        hyper::service::service_fn(move |req: Request<Incoming>| tower_service.clone().call(req));

    let graceful = hyper_util::server::graceful::GracefulShutdown::new();
    // when this signal completes, start shutdown
    let mut signal = std::pin::pin!(shutdown_signal());

    // Run the service with hyper
    loop {
        tokio::select! {
            Ok((stream, _addr)) = listener.accept() => {
                let io = TokioIo::new(stream);
                let conn = http1::Builder::new().serve_connection(io, hyper_service.clone());
                // watch this connection
                let fut = graceful.watch(conn);
                tokio::spawn(async move {
                    if let Err(e) = fut.await {
                        eprintln!("Error serving connection: {:?}", e);
                    }
                });
            },

            _ = &mut signal => {
                drop(listener);
                eprintln!("Graceful shutdown signal received");
                // stop the accept loop
                break;
            }
        }
    }

    tokio::select! {
        _ = graceful.shutdown() => {
            tracing::info!("All connections gracefully closed");
        },
        _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {
            tracing::error!("Timed out wait for all connections to close");
        }
    }

    tracing::info!("Goodbye!");

    Ok(())
}

async fn proxy(req: axum::extract::Request) -> Result<Response, hyper::Error> {
    tracing::trace!(?req);

    if let Some(host_addr) = req.uri().authority().map(|auth| auth.to_string()) {
        tokio::task::spawn(async move {
            match hyper::upgrade::on(req).await {
                Ok(upgraded) => {
                    if let Err(e) = tunnel(upgraded, host_addr).await {
                        tracing::warn!("Server io error: {}", e);
                    };
                }
                Err(e) => tracing::warn!("Upgrade error: {}", e),
            }
        });

        Ok(Response::new(Body::empty()))
    } else {
        tracing::warn!("CONNECT host is not socket addr: {:?}", req.uri());
        Ok((
            StatusCode::BAD_REQUEST,
            "CONNECT must be to a socket address",
        )
            .into_response())
    }
}

async fn tunnel(upgraded: Upgraded, addr: String) -> std::io::Result<()> {
    let mut server = TcpStream::connect(addr).await?;
    let mut upgraded = TokioIo::new(upgraded);

    let (from_client, from_server) =
        tokio::io::copy_bidirectional(&mut upgraded, &mut server).await?;

    tracing::debug!(
        "Client wrote {} bytes and received {} bytes",
        from_client,
        from_server
    );

    Ok(())
}

async fn shutdown_signal() {
    macro_rules! select_sigs {
        ($($name:ident),* $(,)?) => {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Received ctrl_c, exiting");
                }
                $(
                    _ = async {
                        tokio::signal::unix::signal(
                            tokio::signal::unix::SignalKind::$name()
                        )
                        .unwrap()
                        .recv()
                        .await
                    } => {
                        tracing::info!("Received {}, exiting", stringify!($name));
                    }
                )*
            }
        };
    }

    select_sigs!(terminate, hangup, interrupt, quit,);
}
