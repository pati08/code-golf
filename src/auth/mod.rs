pub mod handlers;

use axum::{
    extract::FromRequestParts,
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
    response::Redirect,
};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use sqlx::Row;
use tower_sessions::Session;

const SESSION_USER_KEY: &str = "user";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentUser {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
}

pub struct OptionalUser(pub Option<CurrentUser>);
pub struct RequiredUser(pub CurrentUser);
pub struct RequiredAdmin(pub CurrentUser);

async fn get_user_from_session(parts: &mut Parts) -> Option<CurrentUser> {
    let session = Session::from_request_parts(parts, &()).await.ok()?;
    session.get::<CurrentUser>(SESSION_USER_KEY).await.ok()?
}

impl<S> FromRequestParts<S> for OptionalUser
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let user = get_user_from_session(parts).await;
        Ok(OptionalUser(user))
    }
}

impl<S> FromRequestParts<S> for RequiredUser
where
    S: Send + Sync,
{
    type Rejection = Redirect;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        match get_user_from_session(parts).await {
            Some(user) => Ok(RequiredUser(user)),
            None => Err(Redirect::to("/login")),
        }
    }
}

impl<S> FromRequestParts<S> for RequiredAdmin
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        match get_user_from_session(parts).await {
            Some(user) if user.is_admin => Ok(RequiredAdmin(user)),
            Some(_) => Err(StatusCode::FORBIDDEN),
            None => Err(StatusCode::UNAUTHORIZED),
        }
    }
}

/// Extractor for admin API routes that accept `Authorization: Bearer <token>` authentication.
/// The token is validated against the `api_keys` table (active keys only).
pub struct BearerAdmin(pub CurrentUser);

impl FromRequestParts<crate::app::AppState> for BearerAdmin {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &crate::app::AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .ok_or(StatusCode::UNAUTHORIZED)?
            .to_string();

        let key_hash = {
            let mut h = sha2::Sha256::new();
            h.update(token.as_bytes());
            h.finalize()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        };

        let row = sqlx::query(
            "SELECT u.id, u.username, u.is_admin \
             FROM api_keys k \
             JOIN users u ON u.id = k.created_by \
             WHERE k.key_hash = ? AND k.is_active = 1",
        )
        .bind(&key_hash)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

        let _ = sqlx::query(
            "UPDATE api_keys SET last_used_at = datetime('now') WHERE key_hash = ?",
        )
        .bind(&key_hash)
        .execute(&state.db)
        .await;

        let user = CurrentUser {
            id: row.get("id"),
            username: row.get("username"),
            is_admin: row.get::<i64, _>("is_admin") != 0,
        };

        if !user.is_admin {
            return Err(StatusCode::FORBIDDEN);
        }

        Ok(BearerAdmin(user))
    }
}

pub async fn set_session_user(session: &Session, user: &CurrentUser) -> anyhow::Result<()> {
    session
        .insert(SESSION_USER_KEY, user)
        .await
        .map_err(|e| anyhow::anyhow!("Session error: {e}"))?;
    Ok(())
}

pub async fn clear_session(session: &Session) -> anyhow::Result<()> {
    session
        .flush()
        .await
        .map_err(|e| anyhow::anyhow!("Session error: {e}"))?;
    Ok(())
}
