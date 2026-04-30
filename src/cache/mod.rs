use std::sync::Arc;

use moka::future::Cache;

pub type ProblemList = Vec<(String, String, String, i64)>;
pub type TournamentList = Vec<(String, String, bool)>;

/// Cached problem list with filters
#[derive(Debug, Clone)]
pub struct ProblemListCache {
    cache: Arc<Cache<String, ProblemList>>,
}

impl ProblemListCache {
    pub async fn new(ttl_seconds: u64) -> Self {
        let cache = Cache::builder()
            .max_capacity(1000)
            .time_to_live(std::time::Duration::from_secs(ttl_seconds))
            .build();
        Self {
            cache: Arc::new(cache),
        }
    }

    pub async fn get(&self, key: &str) -> Option<ProblemList> {
        self.cache.get(key).await
    }

    pub async fn insert(&self, key: &str, value: ProblemList) {
        self.cache.insert(key.to_string(), value).await;
    }

    pub async fn invalidate(&self, key: &str) {
        self.cache.remove(key).await;
    }

    pub fn invalidate_all(&self) {
        self.cache.invalidate_all();
    }
}

/// Cached tournament data
#[derive(Debug, Clone)]
pub struct TournamentCache {
    pub list_cache: Arc<Cache<String, TournamentList>>,
    pub active_slug_cache: Arc<Cache<String, Option<String>>>,
    /// Full tournament data for the list page (JSON-serialized context values)
    pub full_list_cache: Arc<Cache<String, String>>,
}

impl TournamentCache {
    pub async fn new(ttl_seconds: u64) -> Self {
        let list_cache = Cache::builder()
            .max_capacity(100)
            .time_to_live(std::time::Duration::from_secs(ttl_seconds))
            .build();
        let active_slug_cache = Cache::builder()
            .max_capacity(10)
            .time_to_live(std::time::Duration::from_secs(ttl_seconds))
            .build();
        let full_list_cache = Cache::builder()
            .max_capacity(20)
            .time_to_live(std::time::Duration::from_secs(ttl_seconds))
            .build();
        Self {
            list_cache: Arc::new(list_cache),
            active_slug_cache: Arc::new(active_slug_cache),
            full_list_cache: Arc::new(full_list_cache),
        }
    }

    pub async fn get_list(&self) -> Option<TournamentList> {
        self.list_cache.get("all").await
    }

    pub async fn set_list(&self, tournaments: TournamentList) {
        self.list_cache.insert("all".to_string(), tournaments).await;
    }

    pub async fn get_active_slug(&self) -> Option<Option<String>> {
        self.active_slug_cache.get("active").await
    }

    pub async fn set_active_slug(&self, slug: Option<String>) {
        self.active_slug_cache.insert("active".to_string(), slug).await;
    }

    pub async fn get_full_list(&self) -> Option<String> {
        self.full_list_cache.get("full").await
    }

    pub async fn set_full_list(&self, tournaments_json: String) {
        self.full_list_cache
            .insert("full".to_string(), tournaments_json)
            .await;
    }

    pub fn invalidate_all(&self) {
        self.list_cache.invalidate_all();
        self.active_slug_cache.invalidate_all();
        self.full_list_cache.invalidate_all();
    }
}

/// Combined cache manager
#[derive(Clone)]
pub struct AppCache {
    pub problem_list: ProblemListCache,
    pub tournament: TournamentCache,
}

impl AppCache {
    pub async fn new(ttl_seconds: u64) -> Self {
        Self {
            problem_list: ProblemListCache::new(ttl_seconds).await,
            tournament: TournamentCache::new(ttl_seconds).await,
        }
    }

    /// Build a cache key for the anonymous problem list (no user-specific solved flags).
    pub fn anon_problem_list_key(tournament: &str, difficulty: Option<&str>) -> String {
        match difficulty {
            Some(d) => format!("anon::{tournament}::{d}"),
            None => format!("anon::{tournament}::"),
        }
    }

    /// Invalidate all caches that could be affected by problem CRUD / publishing changes.
    pub fn invalidate_problems(&self) {
        self.problem_list.invalidate_all();
    }

    /// Invalidate all caches that could be affected by tournament CRUD / active-slug changes.
    pub fn invalidate_tournaments(&self) {
        self.tournament.invalidate_all();
    }

    /// Invalidate the active tournament slug cache only.
    pub fn invalidate_active_slug(&self) {
        self.tournament.active_slug_cache.invalidate_all();
    }
}
