use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolEffect, parse_args, schema_for};
use crate::model::ToolOutput;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Args {
    action: Action,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    id: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Action {
    Add,
    List,
    Complete,
}

pub struct TodoTool;

#[async_trait]
impl Tool for TodoTool {
    fn name(&self) -> &'static str {
        "todo"
    }
    fn description(&self) -> &'static str {
        "Add, list, or complete todos isolated to the current user and session."
    }
    fn schema(&self) -> Value {
        schema_for::<Args>()
    }
    fn validate(&self, value: &Value) -> Result<(), String> {
        let args = parse_args::<Args>(value)?;
        match args.action {
            Action::Add if args.title.as_deref().is_none_or(str::is_empty) => {
                Err("title is required for add".into())
            }
            Action::Complete if args.id.is_none() => Err("id is required for complete".into()),
            _ => Ok(()),
        }
    }
    fn effect(&self, value: &Value) -> ToolEffect {
        match parse_args::<Args>(value).map(|args| args.action) {
            Ok(Action::List) => ToolEffect::ReadOnly,
            _ => ToolEffect::Mutating,
        }
    }
    async fn execute(&self, context: &ToolContext, value: Value) -> ToolOutput {
        let args = match parse_args::<Args>(&value) {
            Ok(args) => args,
            Err(error) => return ToolOutput::failure("invalid_arguments", error),
        };
        match args.action {
            Action::Add => match context
                .memory
                .todo_add(&context.session, args.title.as_deref().unwrap_or_default())
            {
                Ok(id) => ToolOutput::success(json!({"id": id, "status": "open"})),
                Err(error) => ToolOutput::failure("database_error", error.to_string()),
            },
            Action::List => match context.memory.todo_list(&context.session) {
                Ok(items) => ToolOutput::success(json!({"items": items})),
                Err(error) => ToolOutput::failure("database_error", error.to_string()),
            },
            Action::Complete => match context
                .memory
                .todo_complete(&context.session, args.id.unwrap_or_default())
            {
                Ok(true) => ToolOutput::success(json!({"id": args.id, "status": "completed"})),
                Ok(false) => {
                    ToolOutput::failure("not_found", "todo does not exist in this session")
                }
                Err(error) => ToolOutput::failure("database_error", error.to_string()),
            },
        }
    }
}
