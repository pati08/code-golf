pub mod handlers;

use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::Redirect,
};
use serde::{Deserialize, Serialize};
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
