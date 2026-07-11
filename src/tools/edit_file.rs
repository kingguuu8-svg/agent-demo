use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolEffect, parse_args, read_file::resolve, schema_for};
use crate::model::ToolOutput;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Args {
    path: String,
    old_text: String,
    new_text: String,
}

pub struct EditFileTool;

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &'static str {
        "edit_file"
    }
    fn description(&self) -> &'static str {
        "Create a text file or replace one exact, unique text occurrence. Relative and absolute paths are allowed."
    }
    fn schema(&self) -> Value {
        schema_for::<Args>()
    }
    fn validate(&self, value: &Value) -> Result<(), String> {
        parse_args::<Args>(value).map(|_| ())
    }
    fn effect(&self, _: &Value) -> ToolEffect {
        ToolEffect::Mutating
    }
    async fn execute(&self, context: &ToolContext, value: Value) -> ToolOutput {
        let args = match parse_args::<Args>(&value) {
            Ok(args) => args,
            Err(error) => return ToolOutput::failure("invalid_arguments", error),
        };
        let path = resolve(&context.workspace, &args.path);
        if args.old_text.is_empty() {
            if path.exists() {
                return ToolOutput::failure(
                    "already_exists",
                    "old_text may be empty only when creating a new file",
                );
            }
            if let Some(parent) = path.parent()
                && let Err(error) = tokio::fs::create_dir_all(parent).await
            {
                return ToolOutput::failure("write_error", error.to_string());
            }
            return match tokio::fs::write(&path, args.new_text.as_bytes()).await {
                Ok(()) => ToolOutput::success(
                    json!({"path": path, "created": true, "bytes_written": args.new_text.len()}),
                ),
                Err(error) => ToolOutput::failure("write_error", error.to_string()),
            };
        }
        let original = match tokio::fs::read_to_string(&path).await {
            Ok(value) => value,
            Err(error) => return ToolOutput::failure("read_error", error.to_string()),
        };
        let matches = original.match_indices(&args.old_text).count();
        if matches != 1 {
            return ToolOutput::failure(
                "match_count",
                format!("old_text must match exactly once; found {matches}"),
            );
        }
        let updated = original.replacen(&args.old_text, &args.new_text, 1);
        match tokio::fs::write(&path, updated.as_bytes()).await {
            Ok(()) => ToolOutput::success(json!({
                "path": path, "created": false, "old_bytes": original.len(), "new_bytes": updated.len()
            })),
            Err(error) => ToolOutput::failure("write_error", error.to_string()),
        }
    }
}
