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
        Self {
            list_cache: Arc::new(list_cache),
            active_slug_cache: Arc::new(active_slug_cache),
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
}
