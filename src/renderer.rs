use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use crate::{model::ToolOutput, tools::PreparedCall};

pub trait AgentObserver: Send + Sync {
    fn llm_started(&self, _step: usize) {}
    fn llm_finished(&self, _step: usize, _duration: Duration) {}
    fn tool_started(&self, _call: &PreparedCall) {}
    fn tool_finished(
        &self,
        _call: &PreparedCall,
        _output: &ToolOutput,
        _duration: Duration,
        _cached: bool,
    ) {
    }
    fn context_compacted(&self) {}
}

pub struct NoopObserver;
impl AgentObserver for NoopObserver {}

pub struct ConsoleRenderer {
    verbose: AtomicBool,
}

impl ConsoleRenderer {
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose: AtomicBool::new(verbose),
        }
    }

    pub fn set_verbose(&self, verbose: bool) {
        self.verbose.store(verbose, Ordering::Relaxed);
    }

    pub fn verbose(&self) -> bool {
        self.verbose.load(Ordering::Relaxed)
    }
}

impl AgentObserver for ConsoleRenderer {
    fn llm_started(&self, _: usize) {
        eprintln!("\n● Thinking...");
    }

    fn llm_finished(&self, _: usize, duration: Duration) {
        if self.verbose() {
            eprintln!("  model completed in {} ms", duration.as_millis());
        }
    }

    fn tool_started(&self, call: &PreparedCall) {
        eprintln!("\n→ {}{}", call.name, call_hint(call));
        if self.verbose() {
            eprintln!(
                "{}",
                serde_json::to_string_pretty(&call.arguments)
                    .unwrap_or_else(|_| call.arguments.to_string())
            );
        }
    }

    fn tool_finished(
        &self,
        _: &PreparedCall,
        output: &ToolOutput,
        duration: Duration,
        cached: bool,
    ) {
        let marker = if output.ok { "✓" } else { "✗" };
        let cache = if cached { " (cached)" } else { "" };
        eprintln!("{marker} completed in {} ms{cache}", duration.as_millis());
        if self.verbose() || !output.ok {
            eprintln!(
                "{}",
                serde_json::to_string_pretty(output).unwrap_or_else(|_| "<unavailable>".into())
            );
        }
    }

    fn context_compacted(&self) {
        eprintln!("↻ Session context compacted");
    }
}

fn call_hint(call: &PreparedCall) -> String {
    for key in ["path", "command", "expression", "query", "action"] {
        if let Some(value) = call.arguments.get(key) {
            let text = value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| value.to_string());
            let concise: String = text.chars().take(100).collect();
            return format!("  {concise}");
        }
    }
    String::new()
}
