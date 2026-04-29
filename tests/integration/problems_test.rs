//! Integration tests for problem listing and filtering

mod helpers;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use helpers::*;
use tower::ServiceExt;

use crate::{app::AppState, config::Config, runner::LanguageRegistry};
use minijinja::Environment;
use std::sync::{Arc, RwLock};

async fn create_test_state(pool: sqlx::SqlitePool) -> AppState {
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
async fn test_get_problems_list() {
    let pool = create_test_pool().await;
    
    // Create test problems
    let _problem_id = sqlx::query(
        "INSERT INTO problems (slug, title, description, difficulty, is_published) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("easy-problem")
    .bind("Easy Problem")
    .bind("Simple problem")
    .bind("easy")
    .bind(1i64)
    .execute(&pool)
    .await
    .unwrap();
    
    let _hard_problem_id = sqlx::query(
        "INSERT INTO problems (slug, title, description, difficulty, is_published) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("hard-problem")
    .bind("Hard Problem")
    .bind("Hard problem")
    .bind("hard")
    .bind(1i64)
    .execute(&pool)
    .await
    .unwrap();
    
    let state = create_test_state(pool).await;
    let app = state.create_app();
    
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/problems")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body_str = String::from_utf8_lossy(&body);
    
    // Should contain both problems
    assert!(body_str.contains("Easy Problem"));
    assert!(body_str.contains("Hard Problem"));
}

#[tokio::test]
async fn test_filter_by_difficulty() {
    let pool = create_test_pool().await;
    
    // Create test problems
    sqlx::query(
        "INSERT INTO problems (slug, title, description, difficulty, is_published) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("easy1")
    .bind("Easy 1")
    .bind("Easy problem 1")
    .bind("easy")
    .bind(1i64)
    .execute(&pool)
    .await
    .unwrap();
    
    sqlx::query(
        "INSERT INTO problems (slug, title, description, difficulty, is_published) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("easy2")
    .bind("Easy 2")
    .bind("Easy problem 2")
    .bind("easy")
    .bind(1i64)
    .execute(&pool)
    .await
    .unwrap();
    
    sqlx::query(
        "INSERT INTO problems (slug, title, description, difficulty, is_published) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("medium1")
    .bind("Medium 1")
    .bind("Medium problem")
    .bind("medium")
    .bind(1i64)
    .execute(&pool)
    .await
    .unwrap();
    
    let state = create_test_state(pool).await;
    let app = state.create_app();
    
    // Filter by easy
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/problems?difficulty=easy")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body_str = String::from_utf8_lossy(&body);
    
    // Should only contain easy problems
    assert!(body_str.contains("Easy 1"));
    assert!(body_str.contains("Easy 2"));
    assert!(!body_str.contains("Medium 1"));
}

#[tokio::test]
async fn test_get_problem_detail() {
    let pool = create_test_pool().await;
    
    let problem_id = sqlx::query(
        "INSERT INTO problems (slug, title, description, difficulty, is_published) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("test-problem")
    .bind("Test Problem")
    .bind("This is a test problem description")
    .bind("easy")
    .bind(1i64)
    .execute(&pool)
    .await
    .unwrap();
    
    let language_id = get_default_language_id(&pool).await;
    
    // Add test case
    sqlx::query(
        "INSERT INTO test_cases (problem_id, input, expected_output, is_sample) VALUES (?, ?, ?, ?)",
    )
    .bind(problem_id)
    .bind("1 2")
    .bind("3")
    .bind(1i64)
    .execute(&pool)
    .await
    .unwrap();
    
    let state = create_test_state(pool).await;
    let app = state.create_app();
    
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/problems/test-problem")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body_str = String::from_utf8_lossy(&body);
    
    assert!(body_str.contains("Test Problem"));
    assert!(body_str.contains("1 2"));
    assert!(body_str.contains("3"));
}

#[tokio::test]
async fn test_get_problem_not_found() {
    let pool = create_test_pool().await;
    let state = create_test_state(pool).await;
    let app = state.create_app();
    
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/problems/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_unpublished_problems_hidden() {
    let pool = create_test_pool().await;
    
    // Create published problem
    sqlx::query(
        "INSERT INTO problems (slug, title, description, difficulty, is_published) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("published")
    .bind("Published Problem")
    .bind("Published")
    .bind("easy")
    .bind(1i64)
    .execute(&pool)
    .await
    .unwrap();
    
    // Create unpublished problem
    sqlx::query(
        "INSERT INTO problems (slug, title, description, difficulty, is_published) VALUES (?, ?, ?, ?, ?)",
    )
    .bind("unpublished")
    .bind("Unpublished Problem")
    .bind("Not published")
    .bind("easy")
    .bind(0i64)
    .execute(&pool)
    .await
    .unwrap();
    
    let state = create_test_state(pool).await;
    let app = state.create_app();
    
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/problems")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body_str = String::from_utf8_lossy(&body);
    
    assert!(body_str.contains("Published Problem"));
    assert!(!body_str.contains("Unpublished Problem"));
}
