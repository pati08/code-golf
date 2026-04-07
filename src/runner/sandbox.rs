use std::time::{Duration, Instant};
use tokio::{io::AsyncWriteExt, process::Command, time::timeout};

#[derive(Debug)]
pub struct ExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub wall_time_ms: u64,
    pub timed_out: bool,
}

pub async fn execute(
    run_command: &str,
    file_extension: &str,
    code: &str,
    stdin_input: &str,
    time_limit_ms: u64,
) -> anyhow::Result<ExecutionResult> {
    let tmpdir = tempfile::TempDir::new()?;
    let file_path = tmpdir.path().join(format!("solution.{file_extension}"));
    tokio::fs::write(&file_path, code).await?;

    let cmd_str = run_command.replace("{file}", file_path.to_str().unwrap());
    let parts: Vec<&str> = cmd_str.split_whitespace().collect();
    if parts.is_empty() {
        anyhow::bail!("Empty run command");
    }

    let mut child = Command::new(parts[0])
        .args(&parts[1..])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    // Write stdin and capture the child PID before moving child
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(stdin_input.as_bytes()).await;
        // drop to signal EOF
    }

    let child_id = child.id();
    let start = Instant::now();
    let limit = Duration::from_millis(time_limit_ms);

    match timeout(limit, child.wait_with_output()).await {
        Ok(Ok(output)) => {
            let wall_time_ms = start.elapsed().as_millis() as u64;
            Ok(ExecutionResult {
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                exit_code: output.status.code(),
                wall_time_ms,
                timed_out: false,
            })
        }
        Ok(Err(e)) => Err(e.into()),
        Err(_elapsed) => {
            // Timeout — kill child by PID
            if let Some(pid) = child_id {
                let _ = nix::sys::signal::kill(
                    nix::unistd::Pid::from_raw(pid as i32),
                    nix::sys::signal::Signal::SIGKILL,
                );
            }
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
