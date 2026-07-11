mod calculator;
mod edit_file;
mod read_file;
mod search;
mod shell;
mod todo;

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use async_trait::async_trait;
use serde_json::Value;

use crate::{
    memory::Memory,
    model::{SessionKey, ToolDefinition, ToolFunctionSpec, ToolOutput},
};

pub use calculator::CalculatorTool;
pub use edit_file::EditFileTool;
pub use read_file::ReadFileTool;
pub use search::SearchTool;
pub use shell::ShellTool;
pub use todo::TodoTool;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolEffect {
    ReadOnly,
    Mutating,
}

#[derive(Debug, Clone)]
pub struct ToolContext {
    pub session: SessionKey,
    pub workspace: PathBuf,
    pub memory: Arc<Memory>,
}

#[derive(Debug, Clone)]
pub struct PreparedCall {
    pub index: usize,
    pub id: String,
    pub name: String,
    pub arguments: Value,
    pub effect: ToolEffect,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn schema(&self) -> Value;
    fn validate(&self, arguments: &Value) -> std::result::Result<(), String>;
    fn effect(&self, arguments: &Value) -> ToolEffect;
    async fn execute(&self, context: &ToolContext, arguments: Value) -> ToolOutput;
}

#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        self.tools.insert(tool.name().to_string(), Arc::new(tool));
    }

    pub fn standard() -> Self {
        let mut registry = Self::default();
        registry.register(ShellTool);
        registry.register(ReadFileTool);
        registry.register(EditFileTool);
        registry.register(CalculatorTool);
        registry.register(SearchTool);
        registry.register(TodoTool);
        registry
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        let mut definitions: Vec<_> = self
            .tools
            .values()
            .map(|tool| ToolDefinition {
                kind: "function".into(),
                function: ToolFunctionSpec {
                    name: tool.name().into(),
                    description: tool.description().into(),
                    parameters: tool.schema(),
                },
            })
            .collect();
        definitions.sort_by(|left, right| left.function.name.cmp(&right.function.name));
        definitions
    }
}

pub fn schema_for<T: schemars::JsonSchema>() -> Value {
    serde_json::to_value(schemars::schema_for!(T))
        .unwrap_or_else(|_| serde_json::json!({"type":"object"}))
}

pub fn parse_args<T: serde::de::DeserializeOwned>(value: &Value) -> std::result::Result<T, String> {
    serde_json::from_value(value.clone()).map_err(|error| error.to_string())
}
