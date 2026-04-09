use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Template error: {0}")]
    Template(#[from] minijinja::Error),
    #[error("Not found")]
    NotFound,
    #[allow(dead_code)]
    #[error("Unauthorized")]
    Unauthorized,
    #[allow(dead_code)]
    #[error("Forbidden")]
    Forbidden,
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, title, message, suggestion) = match &self {
            AppError::NotFound => (
                StatusCode::NOT_FOUND,
                "Page Not Found",
                "The resource you're looking for doesn't exist.",
                Some("Check the URL and try again, or return to the home page."),
            ),
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "Unauthorized",
                "You need to be logged in to access this resource.",
                Some("Please log in and try again."),
            ),
            AppError::Forbidden => (
                StatusCode::FORBIDDEN,
                "Access Denied",
                "You don't have permission to access this resource.",
                Some("If you believe this is an error, please contact support."),
            ),
            AppError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                "Bad Request",
                msg.as_str(),
                Some("Please check your input and try again."),
            ),
            AppError::Database(e) => {
                tracing::error!("Database error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database Error",
                    "Something went wrong with the database.",
                    Some("Please try again later."),
                )
            }
            AppError::Template(e) => {
                tracing::error!("Template error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Rendering Error",
                    "Something went wrong while rendering the page.",
                    Some("Please try again later."),
                )
            }
            AppError::Internal(e) => {
                tracing::error!("Internal error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal Server Error",
                    "Something unexpected happened.",
                    Some("Please try again later."),
                )
            }
        };

        let html = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{} - Code Golf</title>
  <link rel="stylesheet" href="/static/style.css">
</head>
<body>
  <nav class="navbar">
    <div class="nav-brand"><a href="/">⛳ Code Golf</a></div>
  </nav>

  <main class="container">
    <div class="error-container">
      <div class="error-code">{}</div>
      <h1 class="error-title">{}</h1>
      <p class="error-message">{}</p>
      {}
      <a href="/" class="btn btn-primary">Go Home</a>
    </div>
  </main>

  <footer class="site-footer">
    <p>Code Golf Platform</p>
  </footer>
</body>
</html>"#,
            status.as_u16(),
            status.as_u16(),
            title,
            message,
            suggestion
                .map(|s| format!(r#"<p class="error-suggestion">{}</p>"#, s))
                .unwrap_or_default()
        );

        (status, Html(html)).into_response()
    }
}
