use std::{
    io::{self, Write},
    str::FromStr,
};

use async_trait::async_trait;

use crate::tools::PreparedCall;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    FullAccess,
    RequireApproval,
}

impl FromStr for PermissionMode {
    type Err = String;
    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "full-access" => Ok(Self::FullAccess),
            "require-approval" => Ok(Self::RequireApproval),
            _ => Err("expected full-access or require-approval".into()),
        }
    }
}

#[async_trait]
pub trait Approver: Send + Sync {
    async fn approve(&self, calls: &[PreparedCall]) -> bool;
}

pub struct StdinApprover;

#[async_trait]
impl Approver for StdinApprover {
    async fn approve(&self, calls: &[PreparedCall]) -> bool {
        if calls.is_empty() {
            return true;
        }
        let calls = calls.to_vec();
        tokio::task::spawn_blocking(move || {
            eprintln!("\nAgent requests {} tool call(s):", calls.len());
            for (index, call) in calls.iter().enumerate() {
                eprintln!(
                    "\n[{}] {}\n{}",
                    index + 1,
                    call.name,
                    pretty(&call.arguments)
                );
            }
            eprint!("\nExecute? [y/N]: ");
            let _ = io::stderr().flush();
            let mut input = String::new();
            io::stdin().read_line(&mut input).is_ok() && input.trim().eq_ignore_ascii_case("y")
        })
        .await
        .unwrap_or(false)
    }
}

fn pretty(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}
