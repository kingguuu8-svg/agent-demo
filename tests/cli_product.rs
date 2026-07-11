use std::{
    collections::VecDeque,
    io::Write,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use mini_coding_agent::{
    Agent, AgentConfig, AgentError, AppConfig, Approver, ConfigStore, CredentialStore, LlmClient,
    Memory, PermissionMode, SessionKey, ToolRegistry,
    commands::{ReplCommand, parse_command},
    config::resolve_api_key,
    model::{ChatMessage, ChatRequest, ChatResponse, Choice, FunctionCall, ToolCall},
    renderer::ConsoleRenderer,
    repl::{Repl, Terminal},
    tools::PreparedCall,
};

struct ScriptedLlm {
    responses: Mutex<VecDeque<ChatMessage>>,
    requests: Mutex<Vec<ChatRequest>>,
}

impl ScriptedLlm {
    fn new(responses: Vec<ChatMessage>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
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
        let message = self
            .responses
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

struct Deny;
#[async_trait]
impl Approver for Deny {
    async fn approve(&self, _: &[PreparedCall]) -> bool {
        false
    }
}

struct FakeTerminal {
    input: VecDeque<String>,
    output: String,
}

impl FakeTerminal {
    fn new(input: &[&str]) -> Self {
        Self {
            input: input.iter().map(|value| value.to_string()).collect(),
            output: String::new(),
        }
    }
}

impl Terminal for FakeTerminal {
    fn read_line(&mut self) -> mini_coding_agent::Result<Option<String>> {
        Ok(self.input.pop_front())
    }

    fn write(&mut self, text: &str) -> mini_coding_agent::Result<()> {
        self.output.push_str(text);
        Ok(())
    }
}

struct FakeCredentials(Mutex<Option<String>>);

impl CredentialStore for FakeCredentials {
    fn get(&self) -> mini_coding_agent::Result<Option<String>> {
        Ok(self.0.lock().unwrap().clone())
    }

    fn set(&self, secret: &str) -> mini_coding_agent::Result<()> {
        *self.0.lock().unwrap() = Some(secret.into());
        Ok(())
    }
}

fn assistant(content: &str) -> ChatMessage {
    ChatMessage::assistant(content)
}

fn edit_call() -> ChatMessage {
    ChatMessage {
        role: "assistant".into(),
        content: None,
        reasoning_content: Some("edit".into()),
        tool_calls: Some(vec![ToolCall {
            id: "edit-1".into(),
            kind: "function".into(),
            function: FunctionCall {
                name: "edit_file".into(),
                arguments: r#"{"path":"changed.txt","old_text":"","new_text":"yes"}"#.into(),
            },
        }]),
        tool_call_id: None,
    }
}

#[test]
fn slash_parser_is_deterministic() {
    assert_eq!(parse_command("hello"), ReplCommand::Message("hello".into()));
    assert_eq!(parse_command("/new"), ReplCommand::New);
    assert_eq!(
        parse_command("/resume abc"),
        ReplCommand::Resume(Some("abc".into()))
    );
    assert_eq!(
        parse_command("/permission full-access"),
        ReplCommand::Permission(Some(PermissionMode::FullAccess))
    );
    assert_eq!(parse_command("/trace on"), ReplCommand::Trace(true));
    assert_eq!(parse_command("/paste"), ReplCommand::Paste);
    assert!(matches!(parse_command("/bad"), ReplCommand::Unknown(_)));
}

#[tokio::test]
async fn paste_mode_submits_one_multiline_request() {
    let temp = tempfile::tempdir().unwrap();
    let memory = Arc::new(Memory::open(temp.path().join("agent.db")).unwrap());
    let llm = Arc::new(ScriptedLlm::new(vec![assistant("done")]));
    let renderer = Arc::new(ConsoleRenderer::new(false));
    let agent = Agent::new(
        llm.clone(),
        memory.clone(),
        ToolRegistry::standard(),
        Arc::new(Deny),
        PermissionMode::RequireApproval,
        temp.path().into(),
        AgentConfig::default(),
    );
    let terminal = FakeTerminal::new(&["/paste", "first line", "second line", ".", "/exit"]);
    let mut repl = Repl::new(
        agent,
        memory,
        terminal,
        "u".into(),
        None,
        temp.path().into(),
        temp.path().join("config.json"),
        renderer,
    )
    .unwrap();

    repl.run().await.unwrap();

    let requests = llm.requests();
    assert_eq!(requests.len(), 1);
    assert!(
        requests[0]
            .messages
            .iter()
            .any(|message| message.content.as_deref() == Some("first line\nsecond line"))
    );
}

#[test]
fn config_never_contains_api_key_and_environment_wins() {
    let temp = tempfile::tempdir().unwrap();
    let store = ConfigStore::at(
        temp.path().join("config.json"),
        temp.path().join("agent.db"),
    );
    let config = AppConfig::default();
    store.save(&config).unwrap();
    let credentials = FakeCredentials(Mutex::new(Some("stored-key".into())));
    let json = std::fs::read_to_string(store.path()).unwrap();
    assert!(!json.contains("stored-key"));
    assert!(!json.to_lowercase().contains("api_key"));
    assert_eq!(
        resolve_api_key(Some("environment-key".into()), &credentials).unwrap(),
        Some("environment-key".into())
    );
    assert_eq!(
        resolve_api_key(None, &credentials).unwrap(),
        Some("stored-key".into())
    );
}

#[test]
fn old_database_is_migrated_with_session_titles() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("old.db");
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE sessions(
                user_id TEXT NOT NULL, session_id TEXT NOT NULL,
                summary TEXT NOT NULL DEFAULT '', compacted_through INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
                PRIMARY KEY(user_id,session_id));",
        )
        .unwrap();
    drop(connection);
    let memory = Memory::open(path).unwrap();
    let key = SessionKey::new("u", "old");
    memory.ensure_session(&key).unwrap();
    memory.set_title_if_empty(&key, "Migrated").unwrap();
    assert_eq!(
        memory.session_title(&key).unwrap().as_deref(),
        Some("Migrated")
    );
}

#[test]
fn recent_sessions_hide_empty_entries_and_sort_by_activity() {
    let temp = tempfile::tempdir().unwrap();
    let memory = Memory::open(temp.path().join("agent.db")).unwrap();
    let empty = SessionKey::new("u", "empty");
    let older = SessionKey::new("u", "older");
    let newer = SessionKey::new("u", "newer");
    memory.ensure_session(&empty).unwrap();
    memory.ensure_session(&older).unwrap();
    memory.set_title_if_empty(&older, "Older").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));
    memory.ensure_session(&newer).unwrap();
    memory.set_title_if_empty(&newer, "Newer").unwrap();
    let sessions = memory.list_sessions("u", 20).unwrap();
    assert_eq!(
        sessions
            .iter()
            .map(|session| session.session_id.as_str())
            .collect::<Vec<_>>(),
        vec!["newer", "older"]
    );
}

#[tokio::test]
async fn slash_commands_never_reach_the_llm() {
    let temp = tempfile::tempdir().unwrap();
    let memory = Arc::new(Memory::open(temp.path().join("agent.db")).unwrap());
    let llm = Arc::new(ScriptedLlm::new(vec![]));
    let renderer = Arc::new(ConsoleRenderer::new(false));
    let agent = Agent::new(
        llm.clone(),
        memory.clone(),
        ToolRegistry::standard(),
        Arc::new(Deny),
        PermissionMode::RequireApproval,
        temp.path().into(),
        AgentConfig::default(),
    );
    let terminal = FakeTerminal::new(&["/help", "/status", "/sessions", "/exit"]);
    let mut repl = Repl::new(
        agent,
        memory,
        terminal,
        "u".into(),
        None,
        temp.path().into(),
        temp.path().join("config.json"),
        renderer,
    )
    .unwrap();
    repl.run().await.unwrap();
    assert!(llm.requests().is_empty());
    assert!(repl.terminal().output.contains("Type /help"));
}

#[tokio::test]
async fn first_message_sets_a_concise_session_title() {
    let temp = tempfile::tempdir().unwrap();
    let memory = Arc::new(Memory::open(temp.path().join("agent.db")).unwrap());
    let llm = Arc::new(ScriptedLlm::new(vec![assistant("answer")]));
    let renderer = Arc::new(ConsoleRenderer::new(false));
    let agent = Agent::new(
        llm,
        memory.clone(),
        ToolRegistry::standard(),
        Arc::new(Deny),
        PermissionMode::RequireApproval,
        temp.path().into(),
        AgentConfig::default(),
    );
    let terminal = FakeTerminal::new(&[
        "This is the first user message that should become a concise session title",
        "/exit",
    ]);
    let mut repl = Repl::new(
        agent,
        memory.clone(),
        terminal,
        "u".into(),
        Some("title-session".into()),
        temp.path().into(),
        temp.path().join("config.json"),
        renderer,
    )
    .unwrap();
    repl.run().await.unwrap();
    let title = memory
        .session_title(&SessionKey::new("u", "title-session"))
        .unwrap()
        .unwrap();
    assert!(title.starts_with("This is the first user message"));
    assert!(title.chars().count() <= 48);
}

#[tokio::test]
async fn resume_replays_previous_session_history() {
    let temp = tempfile::tempdir().unwrap();
    let memory = Arc::new(Memory::open(temp.path().join("agent.db")).unwrap());
    let old = SessionKey::new("u", "old-session");
    memory
        .append_message(&old, &ChatMessage::user("old question"))
        .unwrap();
    memory
        .append_message(&old, &assistant("old answer"))
        .unwrap();
    memory.set_title_if_empty(&old, "Old work").unwrap();
    let llm = Arc::new(ScriptedLlm::new(vec![assistant("continued")]));
    let renderer = Arc::new(ConsoleRenderer::new(false));
    let agent = Agent::new(
        llm.clone(),
        memory.clone(),
        ToolRegistry::standard(),
        Arc::new(Deny),
        PermissionMode::RequireApproval,
        temp.path().into(),
        AgentConfig::default(),
    );
    let terminal = FakeTerminal::new(&["/resume old-session", "follow up", "/exit"]);
    let mut repl = Repl::new(
        agent,
        memory,
        terminal,
        "u".into(),
        None,
        temp.path().into(),
        temp.path().join("config.json"),
        renderer,
    )
    .unwrap();
    repl.run().await.unwrap();
    let requests = llm.requests();
    assert_eq!(requests.len(), 1);
    assert!(
        requests[0]
            .messages
            .iter()
            .any(|message| message.content.as_deref() == Some("old question"))
    );
    assert!(
        requests[0]
            .messages
            .iter()
            .any(|message| message.content.as_deref() == Some("follow up"))
    );
}

#[tokio::test]
async fn resume_without_id_accepts_a_list_number() {
    let temp = tempfile::tempdir().unwrap();
    let memory = Arc::new(Memory::open(temp.path().join("agent.db")).unwrap());
    let saved = SessionKey::new("u", "saved-session");
    memory
        .append_message(&saved, &ChatMessage::user("saved question"))
        .unwrap();
    memory.set_title_if_empty(&saved, "Saved work").unwrap();
    let renderer = Arc::new(ConsoleRenderer::new(false));
    let agent = Agent::new(
        Arc::new(ScriptedLlm::new(vec![])),
        memory.clone(),
        ToolRegistry::standard(),
        Arc::new(Deny),
        PermissionMode::RequireApproval,
        temp.path().into(),
        AgentConfig::default(),
    );
    let terminal = FakeTerminal::new(&["/resume", "1", "/exit"]);
    let mut repl = Repl::new(
        agent,
        memory,
        terminal,
        "u".into(),
        None,
        temp.path().into(),
        temp.path().join("config.json"),
        renderer,
    )
    .unwrap();

    repl.run().await.unwrap();

    assert_eq!(repl.active_session().session_id, "saved-session");
    assert!(repl.terminal().output.contains("1. saved-session"));
}

#[tokio::test]
async fn permission_command_changes_the_next_execution() {
    let temp = tempfile::tempdir().unwrap();
    let memory = Arc::new(Memory::open(temp.path().join("agent.db")).unwrap());
    let llm = Arc::new(ScriptedLlm::new(vec![edit_call(), assistant("done")]));
    let renderer = Arc::new(ConsoleRenderer::new(false));
    let agent = Agent::new(
        llm,
        memory.clone(),
        ToolRegistry::standard(),
        Arc::new(Deny),
        PermissionMode::RequireApproval,
        temp.path().into(),
        AgentConfig::default(),
    );
    let terminal = FakeTerminal::new(&["/permission full-access", "edit", "/exit"]);
    let mut repl = Repl::new(
        agent,
        memory,
        terminal,
        "u".into(),
        None,
        temp.path().into(),
        temp.path().join("config.json"),
        renderer,
    )
    .unwrap();
    repl.run().await.unwrap();
    assert_eq!(
        std::fs::read_to_string(temp.path().join("changed.txt")).unwrap(),
        "yes"
    );
}

#[test]
fn installed_cli_no_args_creates_a_session_and_exits_cleanly() {
    let temp = tempfile::tempdir().unwrap();
    let mut child = (0..10)
        .find_map(|attempt| {
            let result = Command::new(env!("CARGO_BIN_EXE_agent-demo"))
                .current_dir(temp.path())
                .env("DEEPSEEK_API_KEY", "test-only-key")
                .env("AGENT_DEMO_CONFIG_DIR", temp.path())
                .env("AGENT_DEMO_DATA_DIR", temp.path())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn();
            match result {
                Ok(child) => Some(child),
                Err(error) if error.raw_os_error() == Some(32) && attempt < 9 => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    None
                }
                Err(error) => panic!("failed to start installed CLI: {error}"),
            }
        })
        .expect("Windows kept the test executable locked after retries");
    child.stdin.take().unwrap().write_all(b"/exit\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Mini Coding Agent"));
    assert!(temp.path().join("agent.db").exists());
}
