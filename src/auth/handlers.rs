use argon2::{
    Argon2,
    PasswordHash,
    PasswordHasher,
    PasswordVerifier,
    password_hash::SaltString,
    // password_hash::rand_core::OsRng,
};
use axum::{
    Form, debug_handler,
    extract::State,
    response::{Html, Redirect, IntoResponse},
};
use rand::Rng;
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    app::AppState,
    auth::{CurrentUser, clear_session, set_session_user},
    error::AppError,
};

#[derive(Deserialize)]
pub struct RegisterForm {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
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

pub async fn get_register(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let ctx = minijinja::context! { error => Option::<String>::None };
    crate::app::render(&state.templates, "auth/register.html", ctx)
}

#[debug_handler]
pub async fn post_register(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<RegisterForm>,
) -> Result<axum::response::Response, AppError> {
    // Rate limit by email to prevent registration spam
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

pub async fn get_login(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let ctx = minijinja::context! { error => Option::<String>::None };
    crate::app::render(&state.templates, "auth/login.html", ctx)
}

pub async fn post_login(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<LoginForm>,
) -> Result<axum::response::Response, AppError> {
    use sqlx::Row;

    // Rate limit by username to slow password-spray attacks
    if !state.rate_limiters.login.check(&form.username).await {
        let ctx = minijinja::context! { error => "Too many login attempts. Try again later." };
        return Ok(crate::app::render(&state.templates, "auth/login.html", ctx)?.into_response());
    }

    let row =
        sqlx::query("SELECT id, username, password_hash, is_admin FROM users WHERE username = ?")
            .bind(&form.username)
            .fetch_optional(&state.db)
            .await?;

    let row = match row {
        Some(r) => r,
        None => {
            let ctx = minijinja::context! {
                error => "Invalid username or password"
            };
            return Ok(crate::app::render(&state.templates, "auth/login.html", ctx)?.into_response());
        }
    };

    let id: i64 = row.get("id");
    let username: String = row.get("username");
    let password_hash: String = row.get("password_hash");
    let is_admin_val: i64 = row.get("is_admin");
    let is_admin = is_admin_val != 0;

    let parsed_hash = PasswordHash::new(&password_hash)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Password hash parse error: {e}")))?;

    if Argon2::default()
        .verify_password(form.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        let ctx = minijinja::context! {
            error => "Invalid username or password"
        };
        return Ok(crate::app::render(&state.templates, "auth/login.html", ctx)?.into_response());
    }

    let current_user = CurrentUser {
        id,
        username,
        is_admin,
    };
    session.cycle_id().await.map_err(|e| AppError::Internal(anyhow::anyhow!("Session error: {e}")))?;
    set_session_user(&session, &current_user).await?;
    Ok(Redirect::to("/").into_response())
}

pub async fn post_logout(session: Session) -> Result<Redirect, AppError> {
    clear_session(&session).await?;
    Ok(Redirect::to("/"))
}
