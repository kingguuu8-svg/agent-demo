use std::{
    io::{self, Write},
    path::PathBuf,
    sync::Arc,
};

use clap::{Parser, Subcommand, ValueEnum};
use mini_coding_agent::{
    Agent, AgentConfig, DeepSeekClient, Memory, PermissionMode, SessionKey, StdinApprover,
    ToolRegistry,
};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "mini-agent",
    version,
    about = "Minimal DeepSeek coding agent runtime"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
    #[arg(long, global = true)]
    json_logs: bool,
}

#[derive(Subcommand)]
enum Command {
    Chat {
        #[arg(long)]
        user: String,
        #[arg(long)]
        session: String,
        #[arg(long, default_value = ".")]
        workspace: PathBuf,
        #[arg(long, default_value = ".mini-agent.db")]
        database: PathBuf,
        #[arg(long, value_enum, default_value = "require-approval")]
        permission: PermissionArg,
    },
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
    match cli.command {
        Command::Chat {
            user,
            session,
            workspace,
            database,
            permission,
        } => {
            let workspace = workspace.canonicalize()?;
            let memory = Arc::new(Memory::open(database)?);
            let agent = Agent::new(
                Arc::new(DeepSeekClient::from_env()?),
                memory,
                ToolRegistry::standard(),
                Arc::new(StdinApprover),
                permission.into(),
                workspace,
                AgentConfig::default(),
            );
            let key = SessionKey::new(user, session);
            println!("Mini Agent ready. Enter a message; Ctrl+D/Ctrl+Z exits.");
            loop {
                print!("> ");
                io::stdout().flush()?;
                let mut input = String::new();
                if io::stdin().read_line(&mut input)? == 0 {
                    break;
                }
                if input.trim().is_empty() {
                    continue;
                }
                tokio::select! {
                    result = agent.run(key.clone(), input.trim().to_string()) => match result {
                        Ok(reply) => println!("{}\n", reply.content),
                        Err(error) => eprintln!("Agent error: {error}\n"),
                    },
                    _ = tokio::signal::ctrl_c() => eprintln!("Cancelled."),
                }
            }
        }
    }
    Ok(())
}

fn init_tracing(json: bool) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("mini_coding_agent=info"));
    if json {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .init();
    }
}
