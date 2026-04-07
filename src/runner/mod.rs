pub mod languages;
pub mod sandbox;

use sqlx::{Row, SqlitePool};

use crate::db::models::Language;

#[derive(Clone)]
pub struct LanguageRegistry {
    pool: SqlitePool,
}

impl LanguageRegistry {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get_enabled(&self) -> anyhow::Result<Vec<Language>> {
        let rows = sqlx::query(
            "SELECT id, name, display_name, file_extension, run_command, is_enabled
             FROM languages WHERE is_enabled = 1 ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .iter()
            .map(|r| Language {
                id: r.get("id"),
                name: r.get("name"),
                display_name: r.get("display_name"),
                file_extension: r.get("file_extension"),
                run_command: r.get("run_command"),
                is_enabled: r.get::<i64, _>("is_enabled") != 0,
            })
            .collect())
    }

    pub async fn get_by_id(&self, id: i64) -> anyhow::Result<Option<Language>> {
        let row = sqlx::query(
            "SELECT id, name, display_name, file_extension, run_command, is_enabled
             FROM languages WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Language {
            id: r.get("id"),
            name: r.get("name"),
            display_name: r.get("display_name"),
            file_extension: r.get("file_extension"),
            run_command: r.get("run_command"),
            is_enabled: r.get::<i64, _>("is_enabled") != 0,
        }))
    }
}
