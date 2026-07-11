use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use crossterm::{event, terminal};
use rustyline::{DefaultEditor, error::ReadlineError};
use tokio_util::sync::CancellationToken;

use crate::{
    Agent, Memory, SessionKey,
    commands::{ReplCommand, parse_command},
    error::{AgentError, Result},
    permission::PermissionMode,
    renderer::ConsoleRenderer,
};

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

pub trait Terminal {
    fn read_line(&mut self) -> Result<Option<String>>;
    fn write(&mut self, text: &str) -> Result<()>;
    fn supports_escape_interrupt(&self) -> bool {
        false
    }
}

pub struct ConsoleTerminal {
    editor: DefaultEditor,
    interactive: bool,
}

impl ConsoleTerminal {
    pub fn new() -> Result<Self> {
        let editor = DefaultEditor::new()
            .map_err(|error| AgentError::Config(format!("terminal unavailable: {error}")))?;
        Ok(Self {
            editor,
            interactive: io::stdin().is_terminal() && io::stdout().is_terminal(),
        })
    }
}

impl Terminal for ConsoleTerminal {
    fn read_line(&mut self) -> Result<Option<String>> {
        match self.editor.readline("") {
            Ok(input) => {
                if !input.trim().is_empty() {
                    let _ = self.editor.add_history_entry(&input);
                }
                Ok(Some(input))
            }
            Err(ReadlineError::Interrupted) => Ok(Some(String::new())),
            Err(ReadlineError::Eof) => Ok(None),
            Err(error) => Err(AgentError::Config(format!(
                "terminal input failed: {error}"
            ))),
        }
    }

    fn write(&mut self, text: &str) -> Result<()> {
        print!("{text}");
        io::stdout().flush()?;
        Ok(())
    }

    fn supports_escape_interrupt(&self) -> bool {
        self.interactive
    }
}

pub struct Repl<T: Terminal> {
    agent: Agent,
    memory: Arc<Memory>,
    terminal: T,
    user_id: String,
    session: SessionKey,
    workspace: PathBuf,
    config_path: PathBuf,
    renderer: Arc<ConsoleRenderer>,
    show_history_on_start: bool,
}

impl<T: Terminal> Repl<T> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        agent: Agent,
        memory: Arc<Memory>,
        terminal: T,
        user_id: String,
        session_id: Option<String>,
        workspace: PathBuf,
        config_path: PathBuf,
        renderer: Arc<ConsoleRenderer>,
    ) -> Result<Self> {
        let show_history_on_start = session_id.is_some();
        let session = SessionKey::new(&user_id, session_id.unwrap_or_else(new_session_id));
        memory.ensure_session(&session)?;
        Ok(Self {
            agent,
            memory,
            terminal,
            user_id,
            session,
            workspace,
            config_path,
            renderer,
            show_history_on_start,
        })
    }

    pub fn active_session(&self) -> &SessionKey {
        &self.session
    }

    pub fn terminal(&self) -> &T {
        &self.terminal
    }

    pub async fn run(&mut self) -> Result<()> {
        self.banner()?;
        if self.show_history_on_start {
            self.show_history(20)?;
        }
        loop {
            self.terminal.write(&format!(
                "\n[{} | {}] ❯ ",
                self.session.session_id,
                self.agent.permission()
            ))?;
            let Some(line) = self.terminal.read_line()? else {
                self.memory.delete_session_if_empty(&self.session)?;
                self.terminal.write("\n")?;
                return Ok(());
            };
            match parse_command(&line) {
                ReplCommand::Message(message) if message.trim().is_empty() => {}
                ReplCommand::Message(message) => self.handle_message(message).await?,
                ReplCommand::New => self.new_session()?,
                ReplCommand::Resume(session) => self.resume(session)?,
                ReplCommand::Sessions => self.show_sessions()?,
                ReplCommand::History(limit) => self.show_history(limit)?,
                ReplCommand::Permission(permission) => self.permission(permission)?,
                ReplCommand::Paste => {
                    if let Some(message) = self.read_paste()? {
                        self.handle_message(message).await?;
                    }
                }
                ReplCommand::Config => self.terminal.write(&format!(
                    "Configuration: {}\nRun `agent-demo config` to edit it.\n",
                    self.config_path.display()
                ))?,
                ReplCommand::Trace(enabled) => {
                    self.renderer.set_verbose(enabled);
                    self.terminal.write(if enabled {
                        "Detailed trace enabled.\n"
                    } else {
                        "Detailed trace disabled.\n"
                    })?;
                }
                ReplCommand::Status => self.status()?,
                ReplCommand::Help => self.help()?,
                ReplCommand::Exit => {
                    self.memory.delete_session_if_empty(&self.session)?;
                    return Ok(());
                }
                ReplCommand::Unknown(command) => self
                    .terminal
                    .write(&format!("Unknown command: {command}. Type /help.\n"))?,
            }
        }
    }

    fn banner(&mut self) -> Result<()> {
        self.terminal.write(&format!(
            "Mini Coding Agent\nSession: {}\nPermission: {}\nWorkspace: {}\nType /help for commands.\n",
            self.session.session_id,
            self.agent.permission(),
            friendly_path(&self.workspace)
        ))
    }

    async fn handle_message(&mut self, message: String) -> Result<()> {
        self.memory
            .set_title_if_empty(&self.session, &session_title(&message))?;
        self.terminal
            .write("Working... press Esc to stop the current request.\n")?;
        let started = std::time::Instant::now();
        let cancellation = CancellationToken::new();
        let result = if self.terminal.supports_escape_interrupt() {
            let watcher_stop = CancellationToken::new();
            let run =
                self.agent
                    .run_cancellable(self.session.clone(), message, cancellation.clone());
            tokio::pin!(run);
            let watcher = wait_for_escape(cancellation.clone(), watcher_stop.clone());
            tokio::pin!(watcher);
            tokio::select! {
                result = &mut run => {
                    watcher_stop.cancel();
                    watcher.await;
                    result
                }
                _ = &mut watcher => run.await,
            }
        } else {
            self.agent.run(self.session.clone(), message).await
        };
        match result {
            Ok(reply) => self.terminal.write(&format!(
                "\nAgent:\n{}\n\nCompleted in {:.1}s · {} LLM call(s) · {} tool call(s).\n",
                reply.content,
                started.elapsed().as_secs_f64(),
                reply.llm_calls,
                reply.tool_calls
            )),
            Err(AgentError::Cancelled) => self.terminal.write(
                "\nStopped. The session is still available and ready for another request.\n",
            ),
            Err(error) => self.terminal.write(&format!("\nAgent error: {error}\n")),
        }
    }

    fn read_paste(&mut self) -> Result<Option<String>> {
        self.terminal.write(
            "Paste text below. Enter a single `.` line to submit, or `/cancel` to abort.\n",
        )?;
        let mut lines = Vec::new();
        loop {
            let Some(line) = self.terminal.read_line()? else {
                return Ok(None);
            };
            if line == "." {
                break;
            }
            if line == "/cancel" {
                self.terminal.write("Paste cancelled.\n")?;
                return Ok(None);
            }
            lines.push(line);
        }
        let message = lines.join("\n");
        if message.trim().is_empty() {
            self.terminal.write("Nothing to submit.\n")?;
            Ok(None)
        } else {
            Ok(Some(message))
        }
    }

    fn new_session(&mut self) -> Result<()> {
        self.memory.delete_session_if_empty(&self.session)?;
        self.session = SessionKey::new(&self.user_id, new_session_id());
        self.memory.ensure_session(&self.session)?;
        self.terminal
            .write(&format!("Created session {}.\n", self.session.session_id))
    }

    fn resume(&mut self, requested: Option<String>) -> Result<()> {
        let session_id = match requested {
            Some(value) => value,
            None => {
                let sessions = self.memory.list_sessions(&self.user_id, 20)?;
                if sessions.is_empty() {
                    return self.terminal.write("No saved sessions.\n");
                }
                self.write_session_list(&sessions)?;
                self.terminal.write("Select number or enter session ID: ")?;
                let Some(selection) = self.terminal.read_line()? else {
                    return Ok(());
                };
                if let Ok(index) = selection.trim().parse::<usize>() {
                    sessions
                        .get(index.saturating_sub(1))
                        .map(|item| item.session_id.clone())
                        .ok_or_else(|| AgentError::Config("invalid session selection".into()))?
                } else {
                    selection.trim().to_string()
                }
            }
        };
        let key = SessionKey::new(&self.user_id, &session_id);
        if !self.memory.session_exists(&key)? {
            return self
                .terminal
                .write(&format!("Session not found: {session_id}\n"));
        }
        if key != self.session {
            self.memory.delete_session_if_empty(&self.session)?;
        }
        self.session = key;
        self.terminal
            .write(&format!("Resumed session {session_id}.\n"))?;
        self.show_history(20)
    }

    fn show_sessions(&mut self) -> Result<()> {
        let sessions = self.memory.list_sessions(&self.user_id, 20)?;
        if sessions.is_empty() {
            return self.terminal.write("No saved sessions.\n");
        }
        self.write_session_list(&sessions)
    }

    fn write_session_list(&mut self, sessions: &[crate::memory::SessionInfo]) -> Result<()> {
        self.terminal.write("Recent sessions:\n")?;
        for (index, session) in sessions.iter().enumerate() {
            let title = if session.title.is_empty() {
                "(untitled)"
            } else {
                &session.title
            };
            self.terminal.write(&format!(
                "  {}. {}  {}\n",
                index + 1,
                session.session_id,
                title
            ))?;
        }
        Ok(())
    }

    fn show_history(&mut self, limit: usize) -> Result<()> {
        let summary = self.memory.summary(&self.session)?;
        let messages = self.memory.load_active_messages(&self.session)?;
        let conversation: Vec<_> = messages
            .iter()
            .filter(|item| matches!(item.message.role.as_str(), "user" | "assistant"))
            .filter_map(|item| {
                item.message
                    .content
                    .as_deref()
                    .filter(|content| !content.trim().is_empty())
                    .map(|content| (item.message.role.as_str(), content))
            })
            .collect();
        if summary.trim().is_empty() && conversation.is_empty() {
            return self.terminal.write("No conversation history.\n");
        }
        self.terminal.write("\nConversation history\n")?;
        if !summary.trim().is_empty() {
            self.terminal.write(&format!(
                "\nEarlier context (compressed):\n{}\n",
                summary.trim()
            ))?;
        }
        let omitted = conversation.len().saturating_sub(limit);
        if omitted > 0 {
            self.terminal.write(&format!(
                "\n... {omitted} older message(s) hidden; use `/history {}` to show more.\n",
                conversation.len().min(500)
            ))?;
        }
        for (role, content) in conversation.into_iter().skip(omitted) {
            let label = if role == "user" { "You" } else { "Agent" };
            self.terminal
                .write(&format!("\n{label}:\n{}\n", content.trim()))?;
        }
        Ok(())
    }

    fn permission(&mut self, requested: Option<PermissionMode>) -> Result<()> {
        let permission = match requested {
            Some(mode) => mode,
            None => {
                self.terminal.write(&format!(
                    "Current permission: {}\nEnter full-access or require-approval: ",
                    self.agent.permission()
                ))?;
                let Some(value) = self.terminal.read_line()? else {
                    return Ok(());
                };
                value.trim().parse().map_err(AgentError::Config)?
            }
        };
        self.agent.set_permission(permission);
        self.terminal
            .write(&format!("Permission set to {permission}.\n"))
    }

    fn status(&mut self) -> Result<()> {
        let title = self
            .memory
            .session_title(&self.session)?
            .unwrap_or_default();
        self.terminal.write(&format!(
            "Session: {}\nTitle: {}\nPermission: {}\nWorkspace: {}\nTrace: {}\n",
            self.session.session_id,
            if title.is_empty() {
                "(untitled)"
            } else {
                &title
            },
            self.agent.permission(),
            friendly_path(&self.workspace),
            if self.renderer.verbose() { "on" } else { "off" }
        ))
    }

    fn help(&mut self) -> Result<()> {
        self.terminal.write(
            "/new                         create a new session\n\
             /resume [session-id]          list or resume sessions\n\
             /sessions                     list recent sessions\n\
             /history [limit]              show conversation history (default 20)\n\
             /permission [mode]            show or change execution permission\n\
             /paste                        enter a multi-line request\n\
             /trace on|off                 toggle detailed tool output\n\
             /status                       show active state\n\
             /config                       show configuration location\n\
             /help                         show this help\n\
             /exit                         quit\n",
        )
    }
}

async fn wait_for_escape(cancellation: CancellationToken, stop: CancellationToken) {
    let _ = tokio::task::spawn_blocking(move || {
        let mut raw = false;
        while !stop.is_cancelled() && !cancellation.is_cancelled() {
            if crate::permission::approval_active() {
                if raw {
                    let _ = terminal::disable_raw_mode();
                    raw = false;
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
                continue;
            }
            if !raw {
                raw = terminal::enable_raw_mode().is_ok();
            }
            if event::poll(std::time::Duration::from_millis(50)).unwrap_or(false)
                && matches!(event::read(), Ok(event::Event::Key(key)) if key.code == event::KeyCode::Esc)
            {
                cancellation.cancel();
                break;
            }
        }
        if raw {
            let _ = terminal::disable_raw_mode();
        }
    })
    .await;
}

pub fn new_session_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let counter = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("s-{millis:x}-{:x}-{counter:x}", std::process::id())
}

fn session_title(message: &str) -> String {
    let normalized = message.split_whitespace().collect::<Vec<_>>().join(" ");
    normalized.chars().take(48).collect()
}

fn friendly_path(path: &std::path::Path) -> String {
    let value = path.display().to_string();
    value.strip_prefix(r"\\?\").unwrap_or(&value).to_string()
}
