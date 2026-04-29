//! Test helpers for integration tests

use sqlx::{SqlitePool, Row};

/// Create a test database pool
pub async fn create_test_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.expect("Failed to create in-memory DB");
    
    // Run migrations if they exist
    if std::path::Path::new("./migrations").exists() {
        sqlx::migrate!("./migrations").run(&pool).await.expect("Failed to run migrations");
    }
    
    pool
}

/// Create a test user
pub async fn create_test_user(pool: &SqlitePool) -> i64 {
    let hash = argon2::hash_password("password123", argon2::password_hash::SaltString::generate(&mut rand::rng()))
        .expect("Failed to hash password")
        .to_string();
    
    let result = sqlx::query(
        "INSERT INTO users (username, email, password_hash, is_admin) VALUES (?, ?, ?, ?)"
    )
    .bind("testuser")
    .bind("test@example.com")
    .bind(hash)
    .bind(1i64) // is_admin
    .execute(pool)
    .await
    .expect("Failed to create test user");
    
    result.last_insert_rowid()
}

/// Create a test problem
pub async fn create_test_problem(pool: &SqlitePool, user_id: i64) -> i64 {
    let problem_id = sqlx::query(
        "INSERT INTO problems (slug, title, description, difficulty, is_published) VALUES (?, ?, ?, ?, ?)"
    )
    .bind("test-problem")
    .bind("Test Problem")
    .bind("Add two numbers.")
    .bind("easy")
    .bind(1i64)
    .execute(pool)
    .await
    .expect("Failed to create test problem")
    .last_insert_rowid();
    
    // Add a test case
    sqlx::query(
        "INSERT INTO test_cases (problem_id, input, expected_output, is_sample) VALUES (?, ?, ?, ?)"
    )
    .bind(problem_id)
    .bind("1 2")
    .bind("3")
    .bind(1i64)
    .execute(pool)
    .await
    .expect("Failed to create test case");
    
    problem_id
}

/// Create a test submission
pub async fn create_test_submission(
    pool: &SqlitePool,
    user_id: i64,
    problem_id: i64,
    language_id: i64,
) -> i64 {
    let result = sqlx::query(
        "INSERT INTO submissions (user_id, problem_id, language_id, code, byte_count) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(user_id)
    .bind(problem_id)
    .bind(language_id)
    .bind("print(1 + 2)")
    .bind(13i64)
    .execute(pool)
    .await
    .expect("Failed to create test submission");
    
    result.last_insert_rowid()
}

/// Get the default language ID
pub async fn get_default_language_id(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT id FROM languages WHERE name = 'python3'")
        .fetch_one(pool)
        .await
        .expect("Failed to get Python3 language ID")
}

/// Reset the database for a clean test
pub async fn reset_database(pool: &SqlitePool) {
    // Delete all data
    sqlx::query("DELETE FROM submissions").execute(pool).await.expect("Failed to reset submissions");
    sqlx::query("DELETE FROM best_submissions").execute(pool).await.expect("Failed to reset best_submissions");
    sqlx::query("DELETE FROM test_cases").execute(pool).await.expect("Failed to reset test_cases");
    sqlx::query("DELETE FROM problems").execute(pool).await.expect("Failed to reset problems");
    sqlx::query("DELETE FROM users").execute(pool).await.expect("Failed to reset users");
    sqlx::query("DELETE FROM sessions").execute(pool).await.expect("Failed to reset sessions");
    
    // Re-seed languages
    sqlx::migrate!("./migrations").run(pool).await.expect("Failed to re-seed");
}
