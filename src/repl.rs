use std::{
    io::{self, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

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
}

pub struct ConsoleTerminal;

impl Terminal for ConsoleTerminal {
    fn read_line(&mut self) -> Result<Option<String>> {
        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            Ok(None)
        } else {
            Ok(Some(input.trim_end_matches(['\r', '\n']).to_string()))
        }
    }

    fn write(&mut self, text: &str) -> Result<()> {
        print!("{text}");
        io::stdout().flush()?;
        Ok(())
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
                ReplCommand::Permission(permission) => self.permission(permission)?,
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
        match self.agent.run(self.session.clone(), message).await {
            Ok(reply) => self
                .terminal
                .write(&format!("\nAgent:\n{}\n", reply.content)),
            Err(error) => self.terminal.write(&format!("\nAgent error: {error}\n")),
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
            .write(&format!("Resumed session {session_id}.\n"))
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
             /permission [mode]            show or change execution permission\n\
             /trace on|off                 toggle detailed tool output\n\
             /status                       show active state\n\
             /config                       show configuration location\n\
             /help                         show this help\n\
             /exit                         quit\n",
        )
    }
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
