use std::{
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use clap::{Parser, Subcommand, ValueEnum};
use mini_coding_agent::{
    Agent, AgentConfig, AppConfig, ConfigStore, CredentialStore, DeepSeekClient,
    KeyringCredentialStore, Memory, PermissionMode, SessionKey, StdinApprover, ToolRegistry,
    config::resolve_api_key,
    renderer::{ConsoleRenderer, NoopObserver},
    repl::{ConsoleTerminal, Repl, new_session_id},
};
use serde_json::json;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "agent-demo", version, about = "Minimal DeepSeek coding agent")]
struct Cli {
    /// Open the interactive configuration wizard (alias for `config`).
    #[arg(long)]
    config: bool,
    /// Emit structured runtime logs to stderr.
    #[arg(long, global = true)]
    json_logs: bool,
    /// Start or continue this session instead of creating one.
    #[arg(long, global = true)]
    session: Option<String>,
    /// Override the configured permission for this process.
    #[arg(long, global = true, value_enum)]
    permission: Option<PermissionArg>,
    /// Override the configured workspace.
    #[arg(long, global = true)]
    workspace: Option<PathBuf>,
    /// Override the configured SQLite database.
    #[arg(long, global = true)]
    database: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Configure DeepSeek and local defaults.
    Config,
    /// Run one prompt without entering the REPL.
    Run {
        prompt: String,
        /// Print only a JSON result to stdout.
        #[arg(long)]
        json: bool,
    },
    /// Remove the one-click installation and its PATH entry.
    Uninstall,
}

#[derive(Clone, Copy, ValueEnum)]
enum PermissionArg {
    FullAccess,
    RequireApproval,
}

impl From<PermissionArg> for PermissionMode {
    fn from(value: PermissionArg) -> Self {
        match value {
            PermissionArg::FullAccess => Self::FullAccess,
            PermissionArg::RequireApproval => Self::RequireApproval,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    init_tracing(cli.json_logs);
    let store = ConfigStore::discover()?;
    let credentials = KeyringCredentialStore;

    if matches!(cli.command, Some(Command::Uninstall)) {
        uninstall()?;
        return Ok(());
    }

    if cli.config || matches!(cli.command, Some(Command::Config)) {
        configure(&store, &credentials)?;
        return Ok(());
    }

    let mut config = store.load()?;
    apply_overrides(&cli, &mut config);
    let api_key = match resolve_api_key(std::env::var("DEEPSEEK_API_KEY").ok(), &credentials)? {
        Some(value) => value,
        None if io::stdin().is_terminal() => {
            eprintln!("DeepSeek is not configured. Starting setup.\n");
            configure(&store, &credentials)?;
            config = store.load()?;
            resolve_api_key(std::env::var("DEEPSEEK_API_KEY").ok(), &credentials)?
                .ok_or("API key was not configured")?
        }
        None => return Err("DeepSeek API key is missing; run `agent-demo config`".into()),
    };

    let workspace = absolute_workspace(&config.workspace)?;
    let database = cli
        .database
        .clone()
        .unwrap_or_else(|| store.database_path(&config));
    if let Some(parent) = database.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let memory = Arc::new(Memory::open(database)?);
    let permission = cli
        .permission
        .map(Into::into)
        .unwrap_or(config.default_permission);
    let agent_config = AgentConfig {
        model: config.model.clone(),
        ..AgentConfig::default()
    };
    let llm = Arc::new(DeepSeekClient::new(api_key, &config.base_url)?);

    match cli.command {
        Some(Command::Run { prompt, json }) => {
            let session = SessionKey::new(
                &config.user_id,
                cli.session.clone().unwrap_or_else(new_session_id),
            );
            memory.ensure_session(&session)?;
            memory.set_title_if_empty(&session, &title(&prompt))?;
            let observer: Arc<dyn mini_coding_agent::renderer::AgentObserver> = if json {
                Arc::new(NoopObserver)
            } else {
                Arc::new(ConsoleRenderer::new(false))
            };
            let agent = Agent::new(
                llm,
                memory,
                ToolRegistry::standard(),
                Arc::new(StdinApprover),
                permission,
                workspace,
                agent_config,
            )
            .with_observer(observer);
            let reply = agent.run(session.clone(), prompt).await?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string(&json!({
                        "session_id": session.session_id,
                        "content": reply.content,
                        "llm_calls": reply.llm_calls,
                        "tool_calls": reply.tool_calls,
                    }))?
                );
            } else {
                println!("\nAgent:\n{}", reply.content);
            }
        }
        Some(Command::Config) => unreachable!("handled before runtime initialization"),
        Some(Command::Uninstall) => unreachable!("handled before runtime initialization"),
        None => {
            let renderer = Arc::new(ConsoleRenderer::new(false));
            let agent = Agent::new(
                llm,
                memory.clone(),
                ToolRegistry::standard(),
                Arc::new(StdinApprover),
                permission,
                workspace.clone(),
                agent_config,
            )
            .with_observer(renderer.clone());
            let mut repl = Repl::new(
                agent,
                memory,
                ConsoleTerminal::new()?,
                config.user_id,
                cli.session,
                workspace,
                store.path().to_path_buf(),
                renderer,
            )?;
            repl.run().await?;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::{Command as ProcessCommand, Stdio};

    let local = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is not set")?;
    let install_dir = PathBuf::from(local).join("AgentDemo").join("bin");
    let path_script = format!(
        "$d='{}'; $p=[Environment]::GetEnvironmentVariable('Path','User'); \
         $n=@($p -split ';' | Where-Object {{ $_ -and $_ -ne $d }}) -join ';'; \
         [Environment]::SetEnvironmentVariable('Path',$n,'User')",
        install_dir.display().to_string().replace('\'', "''")
    );
    let status = ProcessCommand::new("powershell.exe")
        .args(["-NoLogo", "-NoProfile", "-Command", &path_script])
        .status()?;
    if !status.success() {
        return Err("could not remove Agent Demo from the user PATH".into());
    }

    let install_root = install_dir.parent().unwrap_or(&install_dir);
    let cleanup = format!(
        "Start-Sleep -Milliseconds 500; Remove-Item -LiteralPath '{}' -Recurse -Force",
        install_root.display().to_string().replace('\'', "''")
    );
    ProcessCommand::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &cleanup,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    println!("Agent Demo was removed from PATH. Open a new terminal to apply the change.");
    println!("Sessions and configuration were preserved.");
    Ok(())
}

#[cfg(not(windows))]
fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    Err("`agent-demo uninstall` is currently available on Windows only".into())
}

fn configure(
    store: &ConfigStore,
    credentials: &dyn CredentialStore,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = store.load()?;
    println!("Agent Demo configuration\n");
    config.model = prompt_default("Model", &config.model)?;
    config.base_url = prompt_default("Base URL", &config.base_url)?;
    config.user_id = prompt_default("Local user ID", &config.user_id)?;
    config.workspace = PathBuf::from(prompt_default(
        "Default workspace",
        &config.workspace.display().to_string(),
    )?);
    let permission = prompt_default(
        "Default permission (require-approval/full-access)",
        &config.default_permission.to_string(),
    )?;
    config.default_permission = permission
        .parse()
        .map_err(|error: String| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let existing = credentials.get()?.is_some();
    let label = if existing {
        "DeepSeek API Key (leave blank to keep current): "
    } else {
        "DeepSeek API Key: "
    };
    let secret = rpassword::prompt_password(label)?;
    if !secret.trim().is_empty() {
        credentials.set(secret.trim())?;
    } else if !existing && std::env::var("DEEPSEEK_API_KEY").is_err() {
        return Err("API key is required (or set DEEPSEEK_API_KEY)".into());
    }
    store.save(&config)?;
    println!("\nConfiguration saved to {}", store.path().display());
    println!("API key stored in the operating-system credential manager.");
    Ok(())
}

fn prompt_default(label: &str, default: &str) -> io::Result<String> {
    print!("{label} [{default}]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let value = input.trim();
    Ok(if value.is_empty() {
        default.to_string()
    } else {
        value.to_string()
    })
}

fn apply_overrides(cli: &Cli, config: &mut AppConfig) {
    if let Some(workspace) = &cli.workspace {
        config.workspace = workspace.clone();
    }
    if let Ok(model) = std::env::var("DEEPSEEK_MODEL") {
        config.model = model;
    }
    if let Ok(base_url) = std::env::var("DEEPSEEK_BASE_URL") {
        config.base_url = base_url;
    }
}

fn absolute_workspace(path: &Path) -> io::Result<PathBuf> {
    path.canonicalize()
}

fn title(prompt: &str) -> String {
    prompt
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(48)
        .collect()
}

fn init_tracing(json: bool) {
    let default = if json {
        "mini_coding_agent=info"
    } else {
        "mini_coding_agent=warn"
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));
    if json {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .with_writer(io::stderr)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .with_writer(io::stderr)
            .init();
    }
}
