use std::sync::Arc;

use sqlx::{Row, SqlitePool};
use tracing::{error, info};

use crate::runner::{LanguageRegistry, sandbox};

/// Normalize line endings and strip trailing whitespace (equivalent to Python's rstrip).
fn normalize(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n").trim_end().to_string()
}

pub async fn run(submission_id: i64, pool: SqlitePool, runner: Arc<LanguageRegistry>) {
    if let Err(e) = judge(submission_id, &pool, &runner).await {
        error!("Judge error for submission {submission_id}: {e}");
        let _ = sqlx::query(
            "UPDATE submissions SET status = 'runtime_error', error_output = ?, judged_at = datetime('now') WHERE id = ?",
        )
        .bind(e.to_string())
        .bind(submission_id)
        .execute(&pool)
        .await;
    }
}

async fn judge(
    submission_id: i64,
    pool: &SqlitePool,
    runner: &LanguageRegistry,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE submissions SET status = 'running' WHERE id = ?")
        .bind(submission_id)
        .execute(pool)
        .await?;

    let sub = sqlx::query(
        "SELECT code, language_id, problem_id, user_id FROM submissions WHERE id = ?",
    )
    .bind(submission_id)
    .fetch_one(pool)
    .await?;

    let code: String = sub.get("code");
    let language_id: i64 = sub.get("language_id");
    let problem_id: i64 = sub.get("problem_id");
    let user_id: i64 = sub.get("user_id");

    let lang = runner
        .get_by_id(language_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Language not found"))?;

    let problem = sqlx::query(
        "SELECT time_limit_ms, memory_limit_kb FROM problems WHERE id = ?",
    )
    .bind(problem_id)
    .fetch_one(pool)
    .await?;

    let time_limit_ms: i64 = problem.get("time_limit_ms");

    let test_cases = sqlx::query(
        "SELECT input, expected_output, is_sample FROM test_cases WHERE problem_id = ? ORDER BY ordinal, id",
    )
    .bind(problem_id)
    .fetch_all(pool)
    .await?;

    if test_cases.is_empty() {
        finalize(pool, submission_id, user_id, problem_id, language_id, "accepted", None, None).await?;
        return Ok(());
    }

    for tc in &test_cases {
        let input: String = normalize(&tc.get::<String, _>("input"));
        let expected_output: String = tc.get("expected_output");
        let is_sample: bool = tc.get::<i64, _>("is_sample") != 0;

        let result = sandbox::execute(
            &lang.run_command,
            &lang.file_extension,
            &code,
            &input,
            time_limit_ms as u64,
        )
        .await?;

        if result.timed_out {
            finalize(
                pool, submission_id, user_id, problem_id, language_id,
                "time_limit", Some("Time limit exceeded".to_string()), None,
            )
            .await?;
            return Ok(());
        }

        if result.exit_code != Some(0) {
            let raw_err = if result.stderr.is_empty() {
                format!("Exit code: {:?}", result.exit_code)
            } else {
                result.stderr.chars().take(2000).collect()
            };

            // Try to format the code and re-run to get line numbers pointing at readable code.
            let formatted = if let Some(fmt_code) = sandbox::format_code(&lang.file_extension, &code).await {
                let fmt_result = sandbox::execute(
                    &lang.run_command,
                    &lang.file_extension,
                    &fmt_code,
                    &input,
                    time_limit_ms as u64,
                )
                .await?;
                if fmt_result.exit_code != Some(0) {
                    let fmt_err = if fmt_result.stderr.is_empty() {
                        format!("Exit code: {:?}", fmt_result.exit_code)
                    } else {
                        fmt_result.stderr.chars().take(2000).collect()
                    };
                    Some((fmt_code, fmt_err))
                } else {
                    None
                }
            } else {
                None
            };

            finalize(
                pool, submission_id, user_id, problem_id, language_id,
                "runtime_error", Some(raw_err), formatted,
            )
            .await?;
            return Ok(());
        }

        let actual = normalize(&result.stdout);
        let expected = normalize(&expected_output);
        if actual != expected {
            let err = if is_sample {
                let details = serde_json::json!({
                    "input": input.chars().take(500).collect::<String>(),
                    "expected": expected.chars().take(500).collect::<String>(),
                    "actual": actual.chars().take(500).collect::<String>(),
                });
                Some(details.to_string())
            } else {
                None
            };
            finalize(pool, submission_id, user_id, problem_id, language_id, "wrong_answer", err, None)
                .await?;
            return Ok(());
        }
    }

    info!("Submission {submission_id} accepted");
    finalize(pool, submission_id, user_id, problem_id, language_id, "accepted", None, None).await?;
    Ok(())
}

async fn finalize(
    pool: &SqlitePool,
    submission_id: i64,
    user_id: i64,
    problem_id: i64,
    language_id: i64,
    status: &str,
    error_output: Option<String>,
    formatted: Option<(String, String)>,
) -> anyhow::Result<()> {
    let (formatted_code, formatted_error_output) = formatted.unzip();
    sqlx::query(
        "UPDATE submissions SET status = ?, error_output = ?, formatted_code = ?, formatted_error_output = ?, judged_at = datetime('now') WHERE id = ?",
    )
    .bind(status)
    .bind(&error_output)
    .bind(&formatted_code)
    .bind(&formatted_error_output)
    .bind(submission_id)
    .execute(pool)
    .await?;

    if status == "accepted" {
        let byte_count: i64 = sqlx::query(
            "SELECT byte_count FROM submissions WHERE id = ?",
        )
        .bind(submission_id)
        .fetch_one(pool)
        .await?
        .get(0);

        sqlx::query(
            r#"INSERT INTO best_submissions (user_id, problem_id, language_id, submission_id, byte_count)
               VALUES (?, ?, ?, ?, ?)
               ON CONFLICT(user_id, problem_id, language_id) DO UPDATE SET
                 submission_id = excluded.submission_id,
                 byte_count = excluded.byte_count
               WHERE excluded.byte_count < best_submissions.byte_count"#,
        )
        .bind(user_id)
        .bind(problem_id)
        .bind(language_id)
        .bind(submission_id)
        .bind(byte_count)
        .execute(pool)
        .await?;
    }

    Ok(())
}
