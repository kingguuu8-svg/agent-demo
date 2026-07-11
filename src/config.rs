use std::{
    fs,
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::{
    error::{AgentError, Result},
    permission::PermissionMode,
};

const KEYRING_SERVICE: &str = "agent-demo.deepseek";
const KEYRING_ACCOUNT: &str = "default";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AppConfig {
    pub model: String,
    pub base_url: String,
    pub user_id: String,
    pub default_permission: PermissionMode,
    pub workspace: PathBuf,
    pub database: Option<PathBuf>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            model: "deepseek-v4-flash".into(),
            base_url: "https://api.deepseek.com".into(),
            user_id: local_user_id(),
            default_permission: PermissionMode::RequireApproval,
            workspace: PathBuf::from("."),
            database: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    config_path: PathBuf,
    default_database: PathBuf,
}

impl ConfigStore {
    pub fn discover() -> Result<Self> {
        if let Some(config_dir) = std::env::var_os("AGENT_DEMO_CONFIG_DIR") {
            let config_dir = PathBuf::from(config_dir);
            let data_dir = std::env::var_os("AGENT_DEMO_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| config_dir.clone());
            return Ok(Self {
                config_path: config_dir.join("config.json"),
                default_database: data_dir.join("agent.db"),
            });
        }
        let dirs = ProjectDirs::from("dev", "agent-demo", "agent-demo").ok_or_else(|| {
            AgentError::Config("cannot determine the user configuration directory".into())
        })?;
        Ok(Self {
            config_path: dirs.config_dir().join("config.json"),
            default_database: dirs.data_local_dir().join("agent.db"),
        })
    }

    pub fn at(config_path: impl Into<PathBuf>, default_database: impl Into<PathBuf>) -> Self {
        Self {
            config_path: config_path.into(),
            default_database: default_database.into(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.config_path
    }

    pub fn load(&self) -> Result<AppConfig> {
        if !self.config_path.exists() {
            return Ok(AppConfig::default());
        }
        let bytes = fs::read(&self.config_path)?;
        serde_json::from_slice(&bytes).map_err(Into::into)
    }

    pub fn save(&self, config: &AppConfig) -> Result<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut bytes = serde_json::to_vec_pretty(config)?;
        bytes.push(b'\n');
        fs::write(&self.config_path, bytes)?;
        Ok(())
    }

    pub fn database_path(&self, config: &AppConfig) -> PathBuf {
        config
            .database
            .clone()
            .unwrap_or_else(|| self.default_database.clone())
    }
}

pub trait CredentialStore: Send + Sync {
    fn get(&self) -> Result<Option<String>>;
    fn set(&self, secret: &str) -> Result<()>;
}

pub struct KeyringCredentialStore;

impl KeyringCredentialStore {
    fn entry(&self) -> Result<keyring::Entry> {
        keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
            .map_err(|error| AgentError::Config(format!("credential store unavailable: {error}")))
    }
}

impl CredentialStore for KeyringCredentialStore {
    fn get(&self) -> Result<Option<String>> {
        match self.entry()?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(AgentError::Config(format!("cannot read API key: {error}"))),
        }
    }

    fn set(&self, secret: &str) -> Result<()> {
        if secret.trim().is_empty() {
            return Err(AgentError::Config("API key cannot be empty".into()));
        }
        self.entry()?
            .set_password(secret)
            .map_err(|error| AgentError::Config(format!("cannot store API key: {error}")))
    }
}

pub fn resolve_api_key(
    environment: Option<String>,
    credentials: &dyn CredentialStore,
) -> Result<Option<String>> {
    if let Some(value) = environment.filter(|value| !value.trim().is_empty()) {
        return Ok(Some(value));
    }
    credentials.get()
}

fn local_user_id() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "local-user".into())
}
