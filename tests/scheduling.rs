use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use mini_coding_agent::{
    Agent, AgentConfig, AgentError, Approver, LlmClient, Memory, PermissionMode, SessionKey,
    ToolRegistry,
    model::{ChatMessage, ChatRequest, ChatResponse, Choice, FunctionCall, ToolCall, ToolOutput},
    tools::{PreparedCall, Tool, ToolContext, ToolEffect},
};
use serde_json::{Value, json};

struct Scripted(Mutex<VecDeque<ChatMessage>>);

#[async_trait]
impl LlmClient for Scripted {
    async fn complete(&self, _: &ChatRequest) -> mini_coding_agent::Result<ChatResponse> {
        let message = self
            .0
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| AgentError::Config("script exhausted".into()))?;
        Ok(ChatResponse {
            choices: vec![Choice {
                message,
                finish_reason: None,
            }],
            usage: None,
        })
    }
}

struct Allow;
#[async_trait]
impl Approver for Allow {
    async fn approve(&self, _: &[PreparedCall]) -> bool {
        true
    }
}

struct ProbeTool {
    effect: ToolEffect,
    active: Arc<AtomicUsize>,
    maximum: Arc<AtomicUsize>,
    order: Arc<Mutex<Vec<usize>>>,
}

#[async_trait]
impl Tool for ProbeTool {
    fn name(&self) -> &'static str {
        "probe"
    }
    fn description(&self) -> &'static str {
        "Test scheduling behavior."
    }
    fn schema(&self) -> Value {
        json!({"type":"object","properties":{"id":{"type":"integer"}},"required":["id"],"additionalProperties":false})
    }
    fn validate(&self, arguments: &Value) -> Result<(), String> {
        arguments
            .get("id")
            .and_then(Value::as_u64)
            .map(|_| ())
            .ok_or_else(|| "id is required".into())
    }
    fn effect(&self, _: &Value) -> ToolEffect {
        self.effect
    }
    async fn execute(&self, _: &ToolContext, arguments: Value) -> ToolOutput {
        let id = arguments["id"].as_u64().unwrap() as usize;
        self.order.lock().unwrap().push(id);
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum.fetch_max(active, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(75)).await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        ToolOutput::success(json!({"id":id}))
    }
}

fn tool_batch() -> ChatMessage {
    ChatMessage {
        role: "assistant".into(),
        content: None,
        reasoning_content: Some("probe".into()),
        tool_calls: Some(vec![
            ToolCall {
                id: "a".into(),
                kind: "function".into(),
                function: FunctionCall {
                    name: "probe".into(),
                    arguments: r#"{"id":1}"#.into(),
                },
            },
            ToolCall {
                id: "b".into(),
                kind: "function".into(),
                function: FunctionCall {
                    name: "probe".into(),
                    arguments: r#"{"id":2}"#.into(),
                },
            },
        ]),
        tool_call_id: None,
    }
}

async fn run_probe(effect: ToolEffect) -> (usize, Vec<usize>) {
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let order = Arc::new(Mutex::new(Vec::new()));
    let mut registry = ToolRegistry::default();
    registry.register(ProbeTool {
        effect,
        active,
        maximum: maximum.clone(),
        order: order.clone(),
    });
    let llm = Arc::new(Scripted(Mutex::new(VecDeque::from([
        tool_batch(),
        ChatMessage::assistant("done"),
    ]))));
    let temp = tempfile::tempdir().unwrap();
    let agent = Agent::new(
        llm,
        Arc::new(Memory::open(temp.path().join("agent.db")).unwrap()),
        registry,
        Arc::new(Allow),
        PermissionMode::FullAccess,
        temp.path().into(),
        AgentConfig::default(),
    );
    agent.run(SessionKey::new("u", "s"), "probe").await.unwrap();
    let observed_order = order.lock().unwrap().clone();
    (maximum.load(Ordering::SeqCst), observed_order)
}

#[tokio::test]
async fn read_only_batch_executes_concurrently() {
    let (maximum, order) = run_probe(ToolEffect::ReadOnly).await;
    assert_eq!(maximum, 2);
    assert_eq!(order, vec![1, 2]);
}

#[tokio::test]
async fn mutating_batch_executes_serially_in_model_order() {
    let (maximum, order) = run_probe(ToolEffect::Mutating).await;
    assert_eq!(maximum, 1);
    assert_eq!(order, vec![1, 2]);
}
