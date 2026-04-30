use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Form,
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    app::AppState,
    auth::OptionalUser,
    error::AppError,
};

#[derive(Deserialize)]
pub struct FeedbackForm {
    pub category: String,
    pub subject: String,
    pub message: String,
    pub page_url: Option<String>,
    pub csrf_token: String,
}

pub async fn get_feedback_form(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    session: Session,
) -> Result<impl IntoResponse, AppError> {
    let csrf_token = crate::csrf::get_or_create_token(&session).await?;
    let ctx = minijinja::context! {
        current_user => user,
        csrf_token,
    };
    crate::app::render(&state.templates, "feedback/form.html", ctx).map(|html| html.into_response())
}

pub async fn post_feedback(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    session: Session,
    Form(form): Form<FeedbackForm>,
) -> Result<impl IntoResponse, AppError> {
    crate::csrf::validate(&session, &form.csrf_token).await?;

    let rate_key = user.as_ref().map(|u| u.id.to_string()).unwrap_or_else(|| "anon".to_string());
    if !state.rate_limiters.feedback.check(rate_key).await {
        let ctx = minijinja::context! {
            error => "Too many feedback submissions. Please try again later.",
            category => &form.category,
            subject => form.subject.trim(),
            message => form.message.trim(),
        };
        return crate::app::render(&state.templates, "feedback/form.html", ctx)
            .map(|html| (StatusCode::TOO_MANY_REQUESTS, html).into_response());
    }

    // Validation
    if form.subject.trim().is_empty() {
        let ctx = minijinja::context! {
            error => "Subject is required",
            category => form.category,
            message => form.message,
        };
        return crate::app::render(&state.templates, "feedback/form.html", ctx)
            .map(|html| (StatusCode::BAD_REQUEST, html).into_response());
    }

    if form.message.trim().is_empty() {
        let ctx = minijinja::context! {
            error => "Message is required",
            category => form.category,
            subject => form.subject,
        };
        return crate::app::render(&state.templates, "feedback/form.html", ctx)
            .map(|html| (StatusCode::BAD_REQUEST, html).into_response());
    }

    if form.message.trim().len() < 10 {
        let ctx = minijinja::context! {
            error => "Message must be at least 10 characters",
            category => form.category,
            subject => form.subject,
        };
        return crate::app::render(&state.templates, "feedback/form.html", ctx)
            .map(|html| (StatusCode::BAD_REQUEST, html).into_response());
    }

    let category = match form.category.as_str() {
        "bug" | "feature" | "general" | "other" => form.category,
        _ => "general".to_string(),
    };

    // Insert feedback
    let user_id = user.as_ref().map(|u| u.id);
    sqlx::query(
        "INSERT INTO feedback (user_id, category, subject, message, page_url) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(user_id)
    .bind(&category)
    .bind(form.subject.trim())
    .bind(form.message.trim())
    .bind(form.page_url)
    .execute(&state.db)
    .await?;

    let ctx = minijinja::context! {};
    crate::app::render(&state.templates, "feedback/success.html", ctx)
        .map(|html| html.into_response())
}
