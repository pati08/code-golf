use tower_sessions::Session;
use crate::error::AppError;

const CSRF_SESSION_KEY: &str = "csrf_token";

pub async fn get_or_create_token(session: &Session) -> Result<String, AppError> {
    if let Ok(Some(token)) = session.get::<String>(CSRF_SESSION_KEY).await {
        return Ok(token);
    }
    use rand::Rng;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let token = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    session
        .insert(CSRF_SESSION_KEY, &token)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Session insert error: {e}")))?;
    Ok(token)
}

pub async fn validate(session: &Session, submitted: &str) -> Result<(), AppError> {
    let expected = session
        .get::<String>(CSRF_SESSION_KEY)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("Session error: {e}")))?
        .ok_or_else(|| AppError::BadRequest("CSRF token missing from session".to_string()))?;
    if expected == submitted {
        Ok(())
    } else {
        Err(AppError::BadRequest("Invalid CSRF token".to_string()))
    }
}
