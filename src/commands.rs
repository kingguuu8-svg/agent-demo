use std::str::FromStr;

use crate::permission::PermissionMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplCommand {
    Message(String),
    New,
    Resume(Option<String>),
    Sessions,
    Permission(Option<PermissionMode>),
    Config,
    Trace(bool),
    Status,
    Help,
    Exit,
    Unknown(String),
}

pub fn parse_command(input: &str) -> ReplCommand {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return ReplCommand::Message(trimmed.into());
    }
    let mut parts = trimmed.split_whitespace();
    let command = parts.next().unwrap_or_default();
    let argument = parts.next();
    if parts.next().is_some() {
        return ReplCommand::Unknown(trimmed.into());
    }
    match command {
        "/new" => ReplCommand::New,
        "/resume" => ReplCommand::Resume(argument.map(str::to_owned)),
        "/sessions" => ReplCommand::Sessions,
        "/permission" => match argument {
            None => ReplCommand::Permission(None),
            Some(value) => PermissionMode::from_str(value)
                .map(|mode| ReplCommand::Permission(Some(mode)))
                .unwrap_or_else(|_| ReplCommand::Unknown(trimmed.into())),
        },
        "/config" => ReplCommand::Config,
        "/trace" => match argument {
            Some("on") => ReplCommand::Trace(true),
            Some("off") => ReplCommand::Trace(false),
            _ => ReplCommand::Unknown(trimmed.into()),
        },
        "/status" => ReplCommand::Status,
        "/help" => ReplCommand::Help,
        "/exit" | "/quit" => ReplCommand::Exit,
        _ => ReplCommand::Unknown(trimmed.into()),
    }
}
