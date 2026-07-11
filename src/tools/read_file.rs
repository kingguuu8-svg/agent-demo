use std::path::PathBuf;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolEffect, parse_args, schema_for};
use crate::model::ToolOutput;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Args {
    path: String,
    #[serde(default = "default_offset")]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
}
fn default_offset() -> usize {
    1
}
fn default_limit() -> usize {
    200
}

pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }
    fn description(&self) -> &'static str {
        "Read a text file with line numbers. Relative paths resolve from the workspace; absolute paths are allowed."
    }
    fn schema(&self) -> Value {
        schema_for::<Args>()
    }
    fn validate(&self, value: &Value) -> Result<(), String> {
        let args = parse_args::<Args>(value)?;
        if args.offset == 0 {
            return Err("offset is 1-based and must be positive".into());
        }
        if args.limit == 0 || args.limit > 2000 {
            return Err("limit must be between 1 and 2000".into());
        }
        Ok(())
    }
    fn effect(&self, _: &Value) -> ToolEffect {
        ToolEffect::ReadOnly
    }
    async fn execute(&self, context: &ToolContext, value: Value) -> ToolOutput {
        let args = match parse_args::<Args>(&value) {
            Ok(args) => args,
            Err(error) => return ToolOutput::failure("invalid_arguments", error),
        };
        let path = resolve(&context.workspace, &args.path);
        let bytes = match tokio::fs::read(&path).await {
            Ok(bytes) => bytes,
            Err(error) => return ToolOutput::failure("read_error", error.to_string()),
        };
        if bytes.iter().take(8192).any(|byte| *byte == 0) {
            return ToolOutput::failure("binary_file", "binary files are not supported");
        }
        let text = String::from_utf8_lossy(&bytes);
        let lines: Vec<_> = text.lines().collect();
        let start = args.offset.saturating_sub(1).min(lines.len());
        let end = (start + args.limit).min(lines.len());
        let mut content = lines[start..end]
            .iter()
            .enumerate()
            .map(|(index, line)| format!("{:>6} | {}", start + index + 1, line))
            .collect::<Vec<_>>()
            .join("\n");
        let mut truncated = false;
        if content.len() > 65_536 {
            content.truncate(floor_char_boundary(&content, 65_536));
            truncated = true;
        }
        ToolOutput::success(json!({
            "path": path, "content": content, "total_lines": lines.len(),
            "next_offset": (end < lines.len()).then_some(end + 1), "truncated": truncated
        }))
    }
}

pub(super) fn resolve(workspace: &std::path::Path, input: &str) -> PathBuf {
    let path = PathBuf::from(input);
    if path.is_absolute() {
        path
    } else {
        workspace.join(path)
    }
}

fn floor_char_boundary(value: &str, mut index: usize) -> usize {
    while !value.is_char_boundary(index) {
        index -= 1;
    }
    index
}
