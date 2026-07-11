use async_trait::async_trait;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolEffect, parse_args, schema_for};
use crate::model::ToolOutput;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Args {
    expression: String,
}

pub struct CalculatorTool;

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &'static str {
        "calculator"
    }
    fn description(&self) -> &'static str {
        "Evaluate a mathematical expression safely."
    }
    fn schema(&self) -> Value {
        schema_for::<Args>()
    }
    fn validate(&self, value: &Value) -> Result<(), String> {
        parse_args::<Args>(value).map(|_| ())
    }
    fn effect(&self, _: &Value) -> ToolEffect {
        ToolEffect::ReadOnly
    }
    async fn execute(&self, _: &ToolContext, value: Value) -> ToolOutput {
        let args = match parse_args::<Args>(&value) {
            Ok(args) => args,
            Err(error) => return ToolOutput::failure("invalid_arguments", error),
        };
        let mut namespace = fasteval::EmptyNamespace;
        match fasteval::ez_eval(&args.expression, &mut namespace) {
            Ok(result) if result.is_finite() => ToolOutput::success(json!({"result": result})),
            Ok(_) => {
                ToolOutput::failure("non_finite_result", "expression produced NaN or infinity")
            }
            Err(error) => ToolOutput::failure("calculation_error", error.to_string()),
        }
    }
}
