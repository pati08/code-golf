use std::env;

/// Application limits and configuration constants
pub const DEFAULT_MAX_CODE_SIZE: usize = 65536;
pub const DEFAULT_TIME_LIMIT_MS: i64 = 5000;
pub const DEFAULT_MEMORY_LIMIT_KB: i64 = 65536;
pub const DEFAULT_SESSION_EXPIRY_DAYS: i64 = 7;
pub const DEFAULT_DATABASE_MAX_CONNECTIONS: u32 = 20;
pub const DEFAULT_DATABASE_MIN_CONNECTIONS: u32 = 5;

/// Cache configuration
pub const DEFAULT_CACHE_TTL_SECONDS: u64 = 300; // 5 minutes

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub max_code_size: usize,
    pub time_limit_ms: i64,
    pub memory_limit_kb: i64,
    pub session_expiry_days: i64,
    pub database_max_connections: u32,
    pub database_min_connections: u32,
    pub cache_ttl_seconds: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: "sqlite://code-golf.db".to_string(),
            host: "0.0.0.0".to_string(),
            port: 3000,
            max_code_size: DEFAULT_MAX_CODE_SIZE,
            time_limit_ms: DEFAULT_TIME_LIMIT_MS,
            memory_limit_kb: DEFAULT_MEMORY_LIMIT_KB,
            session_expiry_days: DEFAULT_SESSION_EXPIRY_DAYS,
            database_max_connections: DEFAULT_DATABASE_MAX_CONNECTIONS,
            database_min_connections: DEFAULT_DATABASE_MIN_CONNECTIONS,
            cache_ttl_seconds: DEFAULT_CACHE_TTL_SECONDS,
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://code-golf.db".to_string()),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            max_code_size: env::var("MAX_CODE_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_MAX_CODE_SIZE),
            time_limit_ms: env::var("TIME_LIMIT_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_TIME_LIMIT_MS),
            memory_limit_kb: env::var("MEMORY_LIMIT_KB")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_MEMORY_LIMIT_KB),
            session_expiry_days: env::var("SESSION_EXPIRY_DAYS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_SESSION_EXPIRY_DAYS),
            database_max_connections: env::var("DATABASE_MAX_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_DATABASE_MAX_CONNECTIONS),
            database_min_connections: env::var("DATABASE_MIN_CONNECTIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_DATABASE_MIN_CONNECTIONS),
            cache_ttl_seconds: env::var("CACHE_TTL_SECONDS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_CACHE_TTL_SECONDS),
        }
    }
}
