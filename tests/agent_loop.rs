use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use mini_coding_agent::{
    Agent, AgentConfig, AgentError, Approver, LlmClient, Memory, PermissionMode, SessionKey,
    ToolRegistry,
    model::{ChatMessage, ChatRequest, ChatResponse, Choice, FunctionCall, ToolCall},
    tools::PreparedCall,
};
use tempfile::TempDir;

struct ScriptedLlm {
    responses: Mutex<VecDeque<ChatResponse>>,
    requests: Mutex<Vec<ChatRequest>>,
}

impl ScriptedLlm {
    fn new(messages: Vec<ChatMessage>) -> Self {
        Self {
            responses: Mutex::new(messages.into_iter().map(response).collect()),
            requests: Mutex::new(Vec::new()),
        }
    }
    fn requests(&self) -> Vec<ChatRequest> {
        self.requests.lock().unwrap().clone()
    }
}

#[async_trait]
impl LlmClient for ScriptedLlm {
    async fn complete(&self, request: &ChatRequest) -> mini_coding_agent::Result<ChatResponse> {
        self.requests.lock().unwrap().push(request.clone());
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| AgentError::Config("scripted LLM exhausted".into()))
    }
}

struct FixedApprover(bool);

#[async_trait]
impl Approver for FixedApprover {
    async fn approve(&self, _: &[PreparedCall]) -> bool {
        self.0
    }
}

fn response(message: ChatMessage) -> ChatResponse {
    ChatResponse {
        choices: vec![Choice {
            message,
            finish_reason: None,
        }],
        usage: None,
    }
}

fn tool_message(id: &str, name: &str, arguments: &str, reasoning: &str) -> ChatMessage {
    ChatMessage {
        role: "assistant".into(),
        content: None,
        reasoning_content: Some(reasoning.into()),
        tool_calls: Some(vec![ToolCall {
            id: id.into(),
            kind: "function".into(),
            function: FunctionCall {
                name: name.into(),
                arguments: arguments.into(),
            },
        }]),
        tool_call_id: None,
    }
}

fn fixture(llm: Arc<dyn LlmClient>, approve: bool) -> (TempDir, Agent, Arc<Memory>) {
    let temp = tempfile::tempdir().unwrap();
    let memory = Arc::new(Memory::open(temp.path().join("agent.db")).unwrap());
    let agent = Agent::new(
        llm,
        memory.clone(),
        ToolRegistry::standard(),
        Arc::new(FixedApprover(approve)),
        PermissionMode::RequireApproval,
        temp.path().to_path_buf(),
        AgentConfig::default(),
    );
    (temp, agent, memory)
}

#[tokio::test]
async fn direct_answer_skips_tools() {
    let llm = Arc::new(ScriptedLlm::new(vec![ChatMessage::assistant("hello")]));
    let (_temp, agent, _memory) = fixture(llm.clone(), true);
    let reply = agent.run(SessionKey::new("a", "one"), "hi").await.unwrap();
    assert_eq!(reply.content, "hello");
    assert_eq!(reply.tool_calls, 0);
    assert_eq!(llm.requests().len(), 1);
}

#[tokio::test]
async fn tool_result_and_reasoning_are_replayed() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        tool_message(
            "call-1",
            "calculator",
            r#"{"expression":"23*19"}"#,
            "I should calculate.",
        ),
        ChatMessage::assistant("437"),
    ]));
    let (_temp, agent, _memory) = fixture(llm.clone(), true);
    let reply = agent
        .run(SessionKey::new("a", "one"), "calculate")
        .await
        .unwrap();
    assert_eq!(reply.content, "437");
    let requests = llm.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].messages.iter().any(|message| {
        message.role == "assistant"
            && message.reasoning_content.as_deref() == Some("I should calculate.")
    }));
    assert!(
        requests[1]
            .messages
            .iter()
            .any(|message| message.role == "tool"
                && message.tool_call_id.as_deref() == Some("call-1"))
    );
}

#[tokio::test]
async fn denial_executes_nothing_and_returns_result() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        tool_message(
            "call-2",
            "edit_file",
            r#"{"path":"blocked.txt","old_text":"","new_text":"no"}"#,
            "I should edit.",
        ),
        ChatMessage::assistant("The edit was denied."),
    ]));
    let (temp, agent, _memory) = fixture(llm.clone(), false);
    let reply = agent
        .run(SessionKey::new("a", "one"), "edit")
        .await
        .unwrap();
    assert!(reply.content.contains("denied"));
    assert!(!temp.path().join("blocked.txt").exists());
    let requests = llm.requests();
    let result = requests[1]
        .messages
        .iter()
        .find(|message| message.tool_call_id.as_deref() == Some("call-2"))
        .unwrap();
    assert!(result.content.as_deref().unwrap().contains("user_denied"));
}

#[tokio::test]
async fn malformed_arguments_never_execute_tool() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        tool_message("call-3", "edit_file", "{not-json", "Try edit."),
        ChatMessage::assistant("Invalid arguments."),
    ]));
    let (temp, agent, _memory) = fixture(llm, true);
    agent
        .run(SessionKey::new("a", "one"), "edit")
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_dir(temp.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "txt"))
            .count(),
        0
    );
}

#[tokio::test]
async fn sessions_do_not_share_todos() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        tool_message(
            "todo-a",
            "todo",
            r#"{"action":"add","title":"one"}"#,
            "Add todo.",
        ),
        ChatMessage::assistant("added"),
        tool_message("todo-b", "todo", r#"{"action":"list"}"#, "List todos."),
        ChatMessage::assistant("empty"),
    ]));
    let (_temp, agent, memory) = fixture(llm, true);
    agent
        .run(SessionKey::new("user", "one"), "add")
        .await
        .unwrap();
    agent
        .run(SessionKey::new("user", "two"), "list")
        .await
        .unwrap();
    assert_eq!(
        memory
            .todo_list(&SessionKey::new("user", "one"))
            .unwrap()
            .len(),
        1
    );
    assert!(
        memory
            .todo_list(&SessionKey::new("user", "two"))
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn tool_limit_stops_before_execution() {
    let llm = Arc::new(ScriptedLlm::new(vec![tool_message(
        "call",
        "calculator",
        r#"{"expression":"1+1"}"#,
        "calculate",
    )]));
    let temp = tempfile::tempdir().unwrap();
    let memory = Arc::new(Memory::open(temp.path().join("agent.db")).unwrap());
    let config = AgentConfig {
        max_tool_calls: 0,
        ..AgentConfig::default()
    };
    let agent = Agent::new(
        llm,
        memory,
        ToolRegistry::standard(),
        Arc::new(FixedApprover(true)),
        PermissionMode::FullAccess,
        PathBuf::from(temp.path()),
        config,
    );
    assert!(matches!(
        agent.run(SessionKey::new("u", "s"), "x").await,
        Err(AgentError::ToolLimit(0))
    ));
}

#[tokio::test]
async fn compaction_counts_as_llm_call_and_preserves_summary() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        ChatMessage::assistant("summary"),
        ChatMessage::assistant("done"),
    ]));
    let temp = tempfile::tempdir().unwrap();
    let memory = Arc::new(Memory::open(temp.path().join("agent.db")).unwrap());
    let key = SessionKey::new("u", "s");
    memory
        .append_message(&key, &ChatMessage::user("old"))
        .unwrap();
    memory
        .append_message(&key, &ChatMessage::assistant("old answer"))
        .unwrap();
    let config = AgentConfig {
        compression_trigger: 1,
        compression_keep_turns: 1,
        ..AgentConfig::default()
    };
    let agent = Agent::new(
        llm,
        memory.clone(),
        ToolRegistry::standard(),
        Arc::new(FixedApprover(true)),
        PermissionMode::FullAccess,
        temp.path().into(),
        config,
    );
    let reply = agent.run(key.clone(), "new").await.unwrap();
    assert_eq!(reply.llm_calls, 2);
    assert_eq!(memory.summary(&key).unwrap(), "summary");
}

#[tokio::test]
async fn unknown_tool_is_recoverable() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        tool_message("unknown", "does_not_exist", "{}", "Try tool."),
        ChatMessage::assistant("I cannot use that tool."),
    ]));
    let (_temp, agent, _memory) = fixture(llm.clone(), true);
    agent.run(SessionKey::new("u", "s"), "try").await.unwrap();
    let requests = llm.requests();
    let result = requests[1]
        .messages
        .iter()
        .find(|message| message.tool_call_id.as_deref() == Some("unknown"))
        .unwrap();
    assert!(result.content.as_deref().unwrap().contains("unknown_tool"));
}

#[tokio::test]
async fn repeated_observation_triggers_no_progress_guard() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        tool_message("one", "calculator", r#"{"expression":"1+1"}"#, "calculate"),
        tool_message("two", "calculator", r#"{"expression":"1+1"}"#, "repeat"),
        tool_message("three", "calculator", r#"{"expression":"1+1"}"#, "repeat"),
    ]));
    let (_temp, agent, _memory) = fixture(llm, true);
    assert!(matches!(
        agent.run(SessionKey::new("u", "s"), "loop").await,
        Err(AgentError::NoProgress)
    ));
}

#[tokio::test]
async fn duplicate_call_id_reuses_side_effect_result() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        tool_message(
            "same-call",
            "todo",
            r#"{"action":"add","title":"only once"}"#,
            "add",
        ),
        ChatMessage::assistant("done"),
        tool_message(
            "same-call",
            "todo",
            r#"{"action":"add","title":"only once"}"#,
            "add again",
        ),
        ChatMessage::assistant("done again"),
    ]));
    let (_temp, agent, memory) = fixture(llm, true);
    let key = SessionKey::new("u", "s");
    agent.run(key.clone(), "first").await.unwrap();
    agent.run(key.clone(), "second").await.unwrap();
    assert_eq!(memory.todo_list(&key).unwrap().len(), 1);
}

#[tokio::test]
async fn full_access_bypasses_denial_approver() {
    let llm = Arc::new(ScriptedLlm::new(vec![
        tool_message(
            "edit",
            "edit_file",
            r#"{"path":"allowed.txt","old_text":"","new_text":"yes"}"#,
            "edit",
        ),
        ChatMessage::assistant("done"),
    ]));
    let temp = tempfile::tempdir().unwrap();
    let agent = Agent::new(
        llm,
        Arc::new(Memory::open(temp.path().join("agent.db")).unwrap()),
        ToolRegistry::standard(),
        Arc::new(FixedApprover(false)),
        PermissionMode::FullAccess,
        temp.path().into(),
        AgentConfig::default(),
    );
    agent.run(SessionKey::new("u", "s"), "edit").await.unwrap();
    assert_eq!(
        std::fs::read_to_string(temp.path().join("allowed.txt")).unwrap(),
        "yes"
    );
}
