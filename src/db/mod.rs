pub mod models;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;

pub async fn create_pool(database_url: &str) -> anyhow::Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect_with(opts)
        .await?;

    Ok(pool)
}

pub async fn run_migrations(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

pub async fn seed_languages(pool: &SqlitePool) -> anyhow::Result<()> {
    use sqlx::Row;
    let count: i64 = sqlx::query("SELECT COUNT(*) as c FROM languages")
        .fetch_one(pool)
        .await?
        .get(0);

    if count > 0 {
        return Ok(());
    }

    let languages = vec![
        ("python3", "Python 3", "py", "/usr/bin/python3 {file}"),
        ("bash", "Bash", "sh", "/usr/bin/bash {file}"),
        ("ruby", "Ruby", "rb", "/usr/bin/ruby {file}"),
        ("perl", "Perl", "pl", "/usr/bin/perl {file}"),
        ("node", "Node.js", "js", "/usr/bin/node {file}"),
        ("lua", "Lua", "lua", "/usr/bin/lua {file}"),
    ];

    for (name, display_name, ext, cmd) in languages {
        sqlx::query(
            "INSERT OR IGNORE INTO languages (name, display_name, file_extension, run_command) VALUES (?, ?, ?, ?)"
        )
        .bind(name)
        .bind(display_name)
        .bind(ext)
        .bind(cmd)
        .execute(pool)
        .await?;
    }

    Ok(())
}
