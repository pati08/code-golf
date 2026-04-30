use argon2::{
    Argon2,
    PasswordHash,
    PasswordHasher,
    PasswordVerifier,
    password_hash::SaltString,
};
use axum::{
    Form, debug_handler,
    extract::State,
    http::HeaderMap,
    response::{Html, Redirect, IntoResponse},
};
use rand::Rng;
use serde::Deserialize;
use std::sync::OnceLock;
use tower_sessions::Session;

use crate::{
    app::AppState,
    auth::{CurrentUser, clear_session, set_session_user},
    error::AppError,
};

static DUMMY_HASH: OnceLock<String> = OnceLock::new();

fn dummy_hash() -> &'static str {
    DUMMY_HASH.get_or_init(|| {
        let mut bytes = [0u8; 16];
        rand::rng().fill_bytes(&mut bytes);
        let salt = SaltString::encode_b64(&bytes).expect("salt too long");
        Argon2::default()
            .hash_password(b"dummy_constant_time_password_xkcd", &salt)
            .unwrap()
            .to_string()
    })
}

fn extract_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-real-ip")
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("").trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

#[derive(Deserialize)]
pub struct RegisterForm {
    pub username: String,
    pub email: String,
    pub password: String,
    pub csrf_token: String,
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
    pub csrf_token: String,
}

/// Validate username: alphanumeric and underscores only, 3-30 chars
fn validate_username(username: &str) -> Result<(), &'static str> {
    if username.trim().is_empty() {
        return Err("Username is required");
    }
    if username.len() < 3 || username.len() > 30 {
        return Err("Username must be 3-30 characters");
    }
    if !username.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err("Username can only contain letters, numbers, and underscores");
    }
    Ok(())
}

/// Validate password: at least 8 characters
fn validate_password(password: &str) -> Result<(), &'static str> {
    if password.len() < 8 {
        return Err("Password must be at least 8 characters");
    }
    Ok(())
}

/// Validate email: basic email format check
fn validate_email(email: &str) -> Result<(), &'static str> {
    let email = email.trim();
    if email.is_empty() {
        return Err("Email is required");
    }
    if email.len() < 5 || email.len() > 254 {
        return Err("Invalid email address");
    }
    // Basic email format check (not RFC 5322 compliant but covers common cases)
    if !email.contains('@') || !email.contains('.') {
        return Err("Invalid email address format");
    }
    Ok(())
}

pub async fn get_register(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>, AppError> {
    let csrf_token = crate::csrf::get_or_create_token(&session).await?;
    let ctx = minijinja::context! { error => Option::<String>::None, csrf_token };
    crate::app::render(&state.templates, "auth/register.html", ctx)
}

#[debug_handler]
pub async fn post_register(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<RegisterForm>,
) -> Result<axum::response::Response, AppError> {
    crate::csrf::validate(&session, &form.csrf_token).await?;

    // Rate limit by IP first, then by email
    let ip = extract_ip(&headers);
    if !state.rate_limiters.register_ip.check(&ip).await {
        let ctx = minijinja::context! { error => "Too many registration attempts. Try again later." };
        return Ok(crate::app::render(&state.templates, "auth/register.html", ctx)?.into_response());
    }
    if !state.rate_limiters.register.check(&form.email).await {
        let ctx = minijinja::context! { error => "Too many registration attempts. Try again later." };
        return Ok(crate::app::render(&state.templates, "auth/register.html", ctx)?.into_response());
    }

    // Validate input
    if let Err(e) = validate_username(&form.username) {
        let ctx = minijinja::context! { error => e };
        return Ok(crate::app::render(&state.templates, "auth/register.html", ctx)?.into_response());
    }
    if let Err(e) = validate_password(&form.password) {
        let ctx = minijinja::context! { error => e };
        return Ok(crate::app::render(&state.templates, "auth/register.html", ctx)?.into_response());
    }
    if let Err(e) = validate_email(&form.email) {
        let ctx = minijinja::context! { error => e };
        return Ok(crate::app::render(&state.templates, "auth/register.html", ctx)?.into_response());
    }

    let salt = {
        let mut rng = rand::rng();
        let mut bytes = [0u8; 16];
        rng.fill_bytes(&mut bytes);
        SaltString::encode_b64(&bytes).expect("salt length too long to handle")
    };
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(form.password.as_bytes(), &salt)
        .map_err(|_| AppError::BadRequest("Password hashing failed".to_string()))?
        .to_string();

    let result = sqlx::query(
        "INSERT INTO users (username, email, password_hash) VALUES (?, ?, ?)",
    )
    .bind(&form.username)
    .bind(&form.email)
    .bind(&password_hash)
    .execute(&state.db)
    .await;

    match result {
        Ok(row) => {
            let user_id = row.last_insert_rowid();

            // Atomically grant admin only if this is the sole user — race-free single statement
            let update = sqlx::query(
                "UPDATE users SET is_admin = 1 WHERE id = ? AND NOT EXISTS (SELECT 1 FROM users WHERE id != ?)",
            )
            .bind(user_id)
            .bind(user_id)
            .execute(&state.db)
            .await?;
            let is_admin = update.rows_affected() > 0;

            let user = CurrentUser {
                id: user_id,
                username: form.username.clone(),
                is_admin,
            };
            session.cycle_id().await.map_err(|e| AppError::Internal(anyhow::anyhow!("Session error: {e}")))?;
            set_session_user(&session, &user).await?;
            Ok(Redirect::to("/").into_response())
        }
        Err(sqlx::Error::Database(e)) if e.message().contains("UNIQUE") => {
            let ctx = minijinja::context! {
                error => "Registration failed. Please try a different username or email."
            };
            Ok(crate::app::render(&state.templates, "auth/register.html", ctx)?.into_response())
        }
        Err(e) => Err(AppError::Database(e)),
    }
}

pub async fn get_login(
    State(state): State<AppState>,
    session: Session,
) -> Result<Html<String>, AppError> {
    let csrf_token = crate::csrf::get_or_create_token(&session).await?;
    let ctx = minijinja::context! { error => Option::<String>::None, csrf_token };
    crate::app::render(&state.templates, "auth/login.html", ctx)
}

pub async fn post_login(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> Result<axum::response::Response, AppError> {
    use sqlx::Row;

    crate::csrf::validate(&session, &form.csrf_token).await?;

    // Rate limit by IP first, then by username
    let ip = extract_ip(&headers);
    if !state.rate_limiters.login_ip.check(&ip).await {
        let ctx = minijinja::context! { error => "Too many login attempts. Try again later." };
        return Ok(crate::app::render(&state.templates, "auth/login.html", ctx)?.into_response());
    }
    if !state.rate_limiters.login.check(&form.username).await {
        let ctx = minijinja::context! { error => "Too many login attempts. Try again later." };
        return Ok(crate::app::render(&state.templates, "auth/login.html", ctx)?.into_response());
    }

    let row =
        sqlx::query("SELECT id, username, password_hash, is_admin FROM users WHERE username = ?")
            .bind(&form.username)
            .fetch_optional(&state.db)
            .await?;

    // Always run Argon2 verify regardless of whether the user was found,
    // to prevent username enumeration via timing differences.
    let (row, hash_str) = match row {
        Some(r) => {
            let h = r.get::<String, _>("password_hash");
            (Some(r), h)
        }
        None => (None, dummy_hash().to_string()),
    };

    let parsed_hash = PasswordHash::new(&hash_str)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Password hash parse error: {e}")))?;
    let verify_ok = Argon2::default()
        .verify_password(form.password.as_bytes(), &parsed_hash)
        .is_ok();

    if row.is_none() || !verify_ok {
        let ctx = minijinja::context! { error => "Invalid username or password" };
        return Ok(crate::app::render(&state.templates, "auth/login.html", ctx)?.into_response());
    }

    let row = row.unwrap();
    let id: i64 = row.get("id");
    let username: String = row.get("username");
    let is_admin = row.get::<i64, _>("is_admin") != 0;

    let current_user = CurrentUser { id, username, is_admin };
    session.cycle_id().await.map_err(|e| AppError::Internal(anyhow::anyhow!("Session error: {e}")))?;
    set_session_user(&session, &current_user).await?;
    Ok(Redirect::to("/").into_response())
}

pub async fn post_logout(session: Session) -> Result<Redirect, AppError> {
    clear_session(&session).await?;
    Ok(Redirect::to("/"))
}
