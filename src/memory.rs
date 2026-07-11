use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::Value;

use crate::{
    error::Result,
    model::{ChatMessage, SessionKey, ToolOutput},
};

#[derive(Debug, Clone)]
pub struct StoredMessage {
    pub seq: i64,
    pub message: ChatMessage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub session_id: String,
    pub title: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct Memory {
    path: PathBuf,
}

impl Memory {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let memory = Self {
            path: path.as_ref().to_path_buf(),
        };
        memory.with_connection(|connection| {
            connection.execute_batch(
                "PRAGMA journal_mode=WAL;
                 PRAGMA foreign_keys=ON;
                 PRAGMA busy_timeout=5000;
                 CREATE TABLE IF NOT EXISTS sessions (
                    user_id TEXT NOT NULL,
                    session_id TEXT NOT NULL,
                    title TEXT NOT NULL DEFAULT '',
                    summary TEXT NOT NULL DEFAULT '',
                    compacted_through INTEGER NOT NULL DEFAULT 0,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    PRIMARY KEY(user_id, session_id)
                 );
                 CREATE TABLE IF NOT EXISTS messages (
                    user_id TEXT NOT NULL,
                    session_id TEXT NOT NULL,
                    seq INTEGER NOT NULL,
                    message_json TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    PRIMARY KEY(user_id, session_id, seq)
                 );
                 CREATE TABLE IF NOT EXISTS todos (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_id TEXT NOT NULL,
                    session_id TEXT NOT NULL,
                    title TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'open',
                    created_at INTEGER NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS tool_runs (
                    user_id TEXT NOT NULL,
                    session_id TEXT NOT NULL,
                    call_id TEXT NOT NULL,
                    tool_name TEXT NOT NULL,
                    args_json TEXT NOT NULL,
                    output_json TEXT NOT NULL,
                    duration_ms INTEGER NOT NULL,
                    created_at INTEGER NOT NULL,
                    PRIMARY KEY(user_id, session_id, call_id)
                 );",
            )?;
            if !has_column(connection, "sessions", "title")? {
                connection.execute(
                    "ALTER TABLE sessions ADD COLUMN title TEXT NOT NULL DEFAULT ''",
                    [],
                )?;
            }
            Ok(())
        })?;
        Ok(memory)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn ensure_session(&self, key: &SessionKey) -> Result<()> {
        self.with_connection(|connection| {
            let now = now_millis();
            connection.execute(
                "INSERT INTO sessions(user_id,session_id,created_at,updated_at)
                 VALUES(?1,?2,?3,?3)
                 ON CONFLICT(user_id,session_id) DO UPDATE SET updated_at=excluded.updated_at",
                params![key.user_id, key.session_id, now],
            )?;
            Ok(())
        })
    }

    pub fn session_exists(&self, key: &SessionKey) -> Result<bool> {
        self.with_connection(|connection| {
            Ok(connection
                .query_row(
                    "SELECT 1 FROM sessions WHERE user_id=?1 AND session_id=?2",
                    params![key.user_id, key.session_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some())
        })
    }

    pub fn set_title_if_empty(&self, key: &SessionKey, title: &str) -> Result<()> {
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE sessions SET title=?3,updated_at=?4
                 WHERE user_id=?1 AND session_id=?2 AND title=''",
                params![key.user_id, key.session_id, title, now_millis()],
            )?;
            Ok(())
        })
    }

    pub fn list_sessions(&self, user_id: &str, limit: usize) -> Result<Vec<SessionInfo>> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT session_id,title,updated_at FROM sessions
                 WHERE user_id=?1 AND (
                    title<>'' OR EXISTS(
                        SELECT 1 FROM messages
                        WHERE messages.user_id=sessions.user_id
                          AND messages.session_id=sessions.session_id
                    )
                 ) ORDER BY updated_at DESC LIMIT ?2",
            )?;
            let rows = statement.query_map(params![user_id, limit as i64], |row| {
                Ok(SessionInfo {
                    session_id: row.get(0)?,
                    title: row.get(1)?,
                    updated_at: row.get(2)?,
                })
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(Into::into)
        })
    }

    pub fn delete_session_if_empty(&self, key: &SessionKey) -> Result<bool> {
        self.with_connection(|connection| {
            Ok(connection.execute(
                "DELETE FROM sessions
                 WHERE user_id=?1 AND session_id=?2
                   AND NOT EXISTS(
                       SELECT 1 FROM messages
                       WHERE messages.user_id=sessions.user_id
                         AND messages.session_id=sessions.session_id
                   )",
                params![key.user_id, key.session_id],
            )? == 1)
        })
    }

    pub fn session_title(&self, key: &SessionKey) -> Result<Option<String>> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT title FROM sessions WHERE user_id=?1 AND session_id=?2",
                    params![key.user_id, key.session_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(Into::into)
        })
    }

    pub fn append_message(&self, key: &SessionKey, message: &ChatMessage) -> Result<i64> {
        self.ensure_session(key)?;
        self.with_connection(|connection| {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let seq: i64 = transaction.query_row(
                "SELECT COALESCE(MAX(seq),0)+1 FROM messages WHERE user_id=?1 AND session_id=?2",
                params![key.user_id, key.session_id],
                |row| row.get(0),
            )?;
            transaction.execute(
                "INSERT INTO messages(user_id,session_id,seq,message_json,created_at)
                 VALUES(?1,?2,?3,?4,?5)",
                params![
                    key.user_id,
                    key.session_id,
                    seq,
                    serde_json::to_string(message)?,
                    now_millis()
                ],
            )?;
            transaction.commit()?;
            Ok(seq)
        })
    }

    pub fn load_active_messages(&self, key: &SessionKey) -> Result<Vec<StoredMessage>> {
        self.with_connection(|connection| {
            let compacted: i64 = connection
                .query_row(
                    "SELECT compacted_through FROM sessions WHERE user_id=?1 AND session_id=?2",
                    params![key.user_id, key.session_id],
                    |row| row.get(0),
                )
                .optional()?
                .unwrap_or(0);
            let mut statement = connection.prepare(
                "SELECT seq,message_json FROM messages
                 WHERE user_id=?1 AND session_id=?2 AND seq>?3 ORDER BY seq",
            )?;
            let rows = statement
                .query_map(params![key.user_id, key.session_id, compacted], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?;
            let mut messages = Vec::new();
            for row in rows {
                let (seq, json) = row?;
                messages.push(StoredMessage {
                    seq,
                    message: serde_json::from_str(&json)?,
                });
            }
            Ok(messages)
        })
    }

    pub fn summary(&self, key: &SessionKey) -> Result<String> {
        self.with_connection(|connection| {
            Ok(connection
                .query_row(
                    "SELECT summary FROM sessions WHERE user_id=?1 AND session_id=?2",
                    params![key.user_id, key.session_id],
                    |row| row.get(0),
                )
                .optional()?
                .unwrap_or_default())
        })
    }

    pub fn save_compaction(&self, key: &SessionKey, summary: &str, through: i64) -> Result<()> {
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE sessions SET summary=?3,compacted_through=?4,updated_at=?5
                 WHERE user_id=?1 AND session_id=?2",
                params![key.user_id, key.session_id, summary, through, now_millis()],
            )?;
            Ok(())
        })
    }

    pub fn cached_tool_output(
        &self,
        key: &SessionKey,
        call_id: &str,
    ) -> Result<Option<ToolOutput>> {
        self.with_connection(|connection| {
            let json: Option<String> = connection.query_row(
                "SELECT output_json FROM tool_runs WHERE user_id=?1 AND session_id=?2 AND call_id=?3",
                params![key.user_id, key.session_id, call_id],
                |row| row.get(0),
            ).optional()?;
            json.map(|value| serde_json::from_str(&value).map_err(Into::into)).transpose()
        })
    }

    pub fn record_tool_run(
        &self,
        key: &SessionKey,
        call_id: &str,
        tool_name: &str,
        args: &Value,
        output: &ToolOutput,
        duration_ms: u128,
    ) -> Result<()> {
        self.with_connection(|connection| {
            connection.execute(
                "INSERT OR IGNORE INTO tool_runs
                 (user_id,session_id,call_id,tool_name,args_json,output_json,duration_ms,created_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    key.user_id,
                    key.session_id,
                    call_id,
                    tool_name,
                    serde_json::to_string(args)?,
                    serde_json::to_string(output)?,
                    duration_ms as i64,
                    now_millis()
                ],
            )?;
            Ok(())
        })
    }

    pub fn todo_add(&self, key: &SessionKey, title: &str) -> Result<i64> {
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO todos(user_id,session_id,title,status,created_at) VALUES(?1,?2,?3,'open',?4)",
                params![key.user_id, key.session_id, title, now_millis()],
            )?;
            Ok(connection.last_insert_rowid())
        })
    }

    pub fn todo_list(&self, key: &SessionKey) -> Result<Vec<Value>> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT id,title,status FROM todos WHERE user_id=?1 AND session_id=?2 ORDER BY id",
            )?;
            let rows = statement.query_map(params![key.user_id, key.session_id], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "title": row.get::<_, String>(1)?,
                    "status": row.get::<_, String>(2)?,
                }))
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(Into::into)
        })
    }

    pub fn todo_complete(&self, key: &SessionKey, id: i64) -> Result<bool> {
        self.with_connection(|connection| {
            Ok(connection.execute(
                "UPDATE todos SET status='completed' WHERE user_id=?1 AND session_id=?2 AND id=?3",
                params![key.user_id, key.session_id, id],
            )? == 1)
        })
    }

    fn with_connection<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T>,
    ) -> Result<T> {
        let mut connection = Connection::open(&self.path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        operation(&mut connection)
    }
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn has_column(connection: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let names = statement.query_map([], |row| row.get::<_, String>(1))?;
    for name in names {
        if name? == column {
            return Ok(true);
        }
    }
    Ok(false)
}
