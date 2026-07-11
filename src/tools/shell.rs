use std::{process::Stdio, time::Duration};

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::process::Command;

use super::{Tool, ToolContext, ToolEffect, parse_args, schema_for};
use crate::model::ToolOutput;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Args {
    command: String,
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
}
fn default_timeout() -> u64 {
    120_000
}

pub struct ShellTool;

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &'static str {
        "shell"
    }
    fn description(&self) -> &'static str {
        "Execute a shell command with full host permissions in the workspace. This is not sandboxed."
    }
    fn schema(&self) -> Value {
        schema_for::<Args>()
    }
    fn validate(&self, value: &Value) -> Result<(), String> {
        let args = parse_args::<Args>(value)?;
        if args.command.trim().is_empty() {
            return Err("command cannot be empty".into());
        }
        if !(100..=600_000).contains(&args.timeout_ms) {
            return Err("timeout_ms must be between 100 and 600000".into());
        }
        Ok(())
    }
    fn effect(&self, _: &Value) -> ToolEffect {
        ToolEffect::Mutating
    }
    async fn execute(&self, context: &ToolContext, value: Value) -> ToolOutput {
        let args = match parse_args::<Args>(&value) {
            Ok(args) => args,
            Err(error) => return ToolOutput::failure("invalid_arguments", error),
        };
        let mut command = platform_command(&args.command);
        command
            .current_dir(&context.workspace)
            .env_remove("DEEPSEEK_API_KEY")
            .env_remove("OPENAI_API_KEY")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let output =
            match tokio::time::timeout(Duration::from_millis(args.timeout_ms), command.output())
                .await
            {
                Ok(Ok(output)) => output,
                Ok(Err(error)) => return ToolOutput::failure("spawn_error", error.to_string()),
                Err(_) => {
                    return ToolOutput::failure(
                        "timeout",
                        format!("command exceeded {} ms", args.timeout_ms),
                    );
                }
            };
        let (stdout, stdout_truncated) = bounded_text(&output.stdout);
        let (stderr, stderr_truncated) = bounded_text(&output.stderr);
        ToolOutput::success(json!({
            "exit_code": output.status.code(), "success": output.status.success(),
            "stdout": stdout, "stderr": stderr,
            "truncated": stdout_truncated || stderr_truncated
        }))
    }
}

#[cfg(windows)]
fn platform_command(command: &str) -> Command {
    let mut process = Command::new("powershell.exe");
    process.args(["-NoProfile", "-NonInteractive", "-Command", command]);
    process
}

#[cfg(not(windows))]
fn platform_command(command: &str) -> Command {
    let mut process = Command::new("/bin/sh");
    process.args(["-c", command]);
    process
}

fn bounded_text(bytes: &[u8]) -> (String, bool) {
    const LIMIT: usize = 65_536;
    let truncated = bytes.len() > LIMIT;
    (
        String::from_utf8_lossy(&bytes[..bytes.len().min(LIMIT)]).into_owned(),
        truncated,
    )
}
