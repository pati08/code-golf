//! Integration tests for authentication

mod helpers;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use helpers::*;
use hyper::Body as HyperBody;
use sqlx::SqlitePool;
use tower::ServiceExt;

use crate::{app::AppState, config::Config, runner::LanguageRegistry};
use minijinja::Environment;
use std::sync::{Arc, RwLock};

/// Create a test application state
async fn create_test_state(pool: SqlitePool) -> AppState {
    let templates = Arc::new(RwLock::new(
        Environment::new()
            .set_loader(minijinja::path_loader("templates"))
            .expect("Failed to load templates"),
    ));
    
    let runner = Arc::new(LanguageRegistry::new(pool.clone()));
    
    let config = Config {
        database_url: "sqlite::memory:".to_string(),
        host: "127.0.0.1".to_string(),
        port: 3000,
        max_code_size: 65536,
        time_limit_ms: 5000,
        memory_limit_kb: 65536,
        session_expiry_days: 7,
        database_max_connections: 20,
        database_min_connections: 5,
    };
    
    AppState {
        db: pool,
        templates,
        config: Arc::new(config),
        runner,
    }
}

#[tokio::test]
async fn test_register_success() {
    let pool = create_test_pool().await;
    let state = create_test_state(pool).await;
    
    let app = state.create_app();
    
    // Create a new user
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/register")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "username=testuser123&email=test@example.com&password=password123",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::FOUND); // Redirect after successful registration
}

#[tokio::test]
async fn test_register_invalid_username() {
    let pool = create_test_pool().await;
    let state = create_test_state(pool).await;
    
    let app = state.create_app();
    
    // Try with too short username
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/register")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "username=ab&email=test@example.com&password=password123",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    
    // Should return 200 with error message (not redirect)
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_register_weak_password() {
    let pool = create_test_pool().await;
    let state = create_test_state(pool).await;
    
    let app = state.create_app();
    
    // Try with weak password
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/register")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "username=testuser123&email=test@example.com&password=weak",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    
    // Should return 200 with error message
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_register_duplicate_username() {
    let pool = create_test_pool().await;
    let state = create_test_state(pool).await;
    
    // Create first user
    sqlx::query(
        "INSERT INTO users (username, email, password_hash, is_admin) VALUES (?, ?, ?, ?)",
    )
    .bind("existinguser")
    .bind("existing@example.com")
    .bind("testhash")
    .bind(1i64)
    .execute(&pool)
    .await
    .unwrap();
    
    let app = state.create_app();
    
    // Try to register same username
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/register")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    "username=existinguser&email=different@example.com&password=password123",
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    
    // Should return 200 with "already taken" error
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = hyper::body::to_bytes(response.into_body())
        .await
        .unwrap();
    assert!(String::from_utf8_lossy(&body).contains("already taken"));
}

#[tokio::test]
async fn test_login_success() {
    let pool = create_test_pool().await;
    
    // Create test user with known password
    let hash = argon2::hash_password(
        "password123",
        argon2::password_hash::SaltString::generate(&mut rand::rng()),
    )
    .expect("Failed to hash password")
    .to_string();
    
    sqlx::query(
        "INSERT INTO users (username, email, password_hash, is_admin) VALUES (?, ?, ?, ?)",
    )
    .bind("testuser")
    .bind("test@example.com")
    .bind(&hash)
    .bind(1i64)
    .execute(&pool)
    .await
    .unwrap();
    
    let state = create_test_state(pool).await;
    let app = state.create_app();
    
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("username=testuser&password=password123"))
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::FOUND); // Redirect after successful login
}

#[tokio::test]
async fn test_login_invalid_credentials() {
    let pool = create_test_pool().await;
    let state = create_test_state(pool).await;
    
    let app = state.create_app();
    
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("username=testuser&password=wrongpassword"))
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK); // Should return page with error
    
    let body = hyper::body::to_bytes(response.into_body())
        .await
        .unwrap();
    assert!(String::from_utf8_lossy(&body).contains("Invalid username or password"));
}
