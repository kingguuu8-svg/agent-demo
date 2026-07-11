use thiserror::Error;

pub type Result<T> = std::result::Result<T, AgentError>;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("DeepSeek API error ({status}): {body}")]
    Api { status: u16, body: String },
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("model returned no answer or tool call")]
    EmptyModelResponse,
    #[error("LLM request limit reached ({0})")]
    LlmLimit(usize),
    #[error("tool call limit reached ({0})")]
    ToolLimit(usize),
    #[error("repeated tool call made no progress")]
    NoProgress,
    #[error("run exceeded {0} seconds")]
    Deadline(u64),
    #[error("request stopped by user")]
    Cancelled,
    #[error("context compaction failed: {0}")]
    Compaction(String),
}
