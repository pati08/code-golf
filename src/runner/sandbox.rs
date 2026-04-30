use std::time::{Duration, Instant};
use tokio::{io::AsyncWriteExt, process::Command, time::timeout};

/// Returns the stdin→stdout formatter command for a given file extension, if one exists.
fn formatter_for(file_extension: &str) -> Option<&'static [&'static str]> {
    match file_extension {
        "py" => Some(&["black", "--quiet", "-"]),
        "sh" => Some(&["shfmt", "-"]),
        "js" => Some(&["prettier", "--stdin-filepath", "solution.js"]),
        "pl" => Some(&["perltidy", "-"]),
        _ => None,
    }
}

/// Try to format `code` using the appropriate formatter for `file_extension`.
/// Returns the formatted code on success, or `None` if no formatter exists or it fails.
pub async fn format_code(file_extension: &str, code: &str) -> Option<String> {
    let parts = formatter_for(file_extension)?;

    let mut child = Command::new(parts[0])
        .args(&parts[1..])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(code.as_bytes()).await;
    }

    let output = timeout(Duration::from_secs(10), child.wait_with_output())
        .await
        .ok()?
        .ok()?;

    if output.status.success() {
        let formatted = String::from_utf8_lossy(&output.stdout).into_owned();
        if formatted.trim().is_empty() { None } else { Some(formatted) }
    } else {
        None
    }
}

#[derive(Debug)]
pub struct ExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    #[allow(dead_code)]
    pub wall_time_ms: u64,
    pub timed_out: bool,
}

pub async fn execute(
    run_command: &str,
    file_extension: &str,
    code: &str,
    stdin_input: &str,
    time_limit_ms: u64,
    memory_limit_kb: u64,
) -> anyhow::Result<ExecutionResult> {
    let tmpdir = tempfile::TempDir::new()?;
    let file_path = tmpdir.path().join(format!("solution.{file_extension}"));
    tokio::fs::write(&file_path, code).await?;

    let cmd_str = run_command.replace("{file}", file_path.to_str().unwrap());
    let parts: Vec<&str> = cmd_str.split_whitespace().collect();
    if parts.is_empty() {
        anyhow::bail!("Empty run command");
    }

    let mut cmd = Command::new(parts[0]);
    cmd.args(&parts[1..])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    // Set resource limits and security restrictions in the child process after
    // fork but before exec, to constrain submitted code without affecting the server.
    #[cfg(target_os = "linux")]
    {
        let mem_bytes = memory_limit_kb.saturating_mul(1024);
        let cpu_secs = (time_limit_ms / 1000).max(1);
        unsafe {
            cmd.pre_exec(move || {
                use nix::sys::resource::{Resource, setrlimit};

                // Virtual memory limit
                setrlimit(Resource::RLIMIT_AS, mem_bytes, mem_bytes)
                    .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;

                // CPU time limit (backup to wall-clock timeout)
                setrlimit(Resource::RLIMIT_CPU, cpu_secs, cpu_secs)
                    .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;

                // Prevent privilege escalation via setuid binaries
                nix::sys::prctl::set_no_new_privs()
                    .map_err(|e| std::io::Error::from_raw_os_error(e as i32))?;

                Ok(())
            });
        }
    }

    let mut child = cmd.spawn()?;

    // Write stdin and close it to signal EOF
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(stdin_input.as_bytes()).await;
    }

    // Collect stdout/stderr in background tasks so we can wait() separately
    let stdout_reader = child.stdout.take().expect("stdout piped");
    let stderr_reader = child.stderr.take().expect("stderr piped");

    let stdout_task = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut buf = Vec::new();
        let mut reader = tokio::io::BufReader::new(stdout_reader);
        reader.read_to_end(&mut buf).await.ok();
        buf
    });
    let stderr_task = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut buf = Vec::new();
        let mut reader = tokio::io::BufReader::new(stderr_reader);
        reader.read_to_end(&mut buf).await.ok();
        buf
    });

    let start = Instant::now();
    let limit = Duration::from_millis(time_limit_ms);

    match timeout(limit, child.wait()).await {
        Ok(Ok(status)) => {
            let wall_time_ms = start.elapsed().as_millis() as u64;
            let stdout_bytes = stdout_task.await.unwrap_or_default();
            let stderr_bytes = stderr_task.await.unwrap_or_default();
            Ok(ExecutionResult {
                stdout: String::from_utf8_lossy(&stdout_bytes).into_owned(),
                stderr: String::from_utf8_lossy(&stderr_bytes).into_owned(),
                exit_code: status.code(),
                wall_time_ms,
                timed_out: false,
            })
        }
        Ok(Err(e)) => Err(e.into()),
        Err(_elapsed) => {
            // Properly kill and reap the child to avoid zombies and PID-recycle races
            let _ = child.kill().await;
            let _ = child.wait().await;
            stdout_task.abort();
            stderr_task.abort();
            Ok(ExecutionResult {
                stdout: String::new(),
                stderr: "Time limit exceeded".to_string(),
                exit_code: None,
                wall_time_ms: time_limit_ms,
                timed_out: true,
            })
        }
    }
}
