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
use sqlx::Row;
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
    if form.username.trim().is_empty() || form.password.len() < 6 {
        let ctx = minijinja::context! {
            error => "Username required and password must be at least 6 characters"
        };
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

    // Check if this is the first user
    let user_count: i64 = sqlx::query("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await?
        .get(0);
    let is_first_user = user_count == 0;

    let result = sqlx::query(
        "INSERT INTO users (username, email, password_hash, is_admin) VALUES (?, ?, ?, ?)",
    )
    .bind(&form.username)
    .bind(&form.email)
    .bind(&password_hash)
    .bind(is_first_user as i64)
    .execute(&state.db)
    .await;

    match result {
        Ok(row) => {
            let user = CurrentUser {
                id: row.last_insert_rowid(),
                username: form.username.clone(),
                is_admin: is_first_user,
            };
            set_session_user(&session, &user).await?;
            Ok(Redirect::to("/").into_response())
        }
        Err(sqlx::Error::Database(e)) if e.message().contains("UNIQUE") => {
            let ctx = minijinja::context! {
                error => "Username or email already taken"
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
    set_session_user(&session, &current_user).await?;
    Ok(Redirect::to("/").into_response())
}

pub async fn post_logout(session: Session) -> Result<Redirect, AppError> {
    clear_session(&session).await?;
    Ok(Redirect::to("/"))
}
