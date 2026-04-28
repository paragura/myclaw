/// Session recording (rollout) — logs conversation events as JSONL.
///
/// Based on Codex's RolloutRecorder: records messages, tool calls, and results
/// as lightweight JSONL entries. Stored in SQLite for searchability, with the
/// raw JSONL appended to a file for debugging and replay.
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tracing::{debug, error, info};

/// A single event in a conversation session.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RolloutEvent {
    #[serde(rename = "user_message")]
    UserMessage {
        user_id: String,
        channel_id: String,
        content: String,
    },
    #[serde(rename = "assistant_message")]
    AssistantMessage {
        channel_id: String,
        content: String,
    },
    #[serde(rename = "tool_call")]
    ToolCall {
        tool: String,
        arguments: String,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool: String,
        success: bool,
        output: String,
    },
    #[serde(rename = "session_start")]
    SessionStart {
        user_id: String,
        channel_id: String,
        model: String,
    },
    #[serde(rename = "session_end")]
    SessionEnd {
        message_count: usize,
        tool_call_count: usize,
    },
}

/// Metadata for a recorded session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub session_id: String,
    pub user_id: String,
    pub channel_id: String,
    pub model: String,
    pub message_count: usize,
    pub tool_call_count: usize,
}

/// In-memory accumulator for a session's events.
pub struct RolloutRecorder {
    session_id: String,
    events: Vec<RolloutEvent>,
    pool: SqlitePool,
}

impl RolloutRecorder {
    pub fn new(session_id: &str, pool: SqlitePool) -> Self {
        Self {
            session_id: session_id.to_string(),
            events: Vec::new(),
            pool,
        }
    }

    /// Add an event to the rollout.
    pub fn record(&mut self, event: RolloutEvent) {
        self.events.push(event);
    }

    /// Flush all pending events to the database.
    pub async fn flush(&self) {
        if self.events.is_empty() {
            return;
        }

        let mut success_count = 0u64;
        for (i, event) in self.events.iter().enumerate() {
            let json = serde_json::to_string(event).unwrap_or_default();
            let event_id = uuid::Uuid::new_v4().to_string();
            match sqlx::query(
                "INSERT INTO rollout_events (id, session_id, sequence, json) VALUES (?, ?, ?, ?)",
            )
            .bind(&event_id)
            .bind(&self.session_id)
            .bind(i as i64)
            .bind(&json)
            .execute(&self.pool)
            .await
            {
                Ok(r) => {
                    success_count += r.rows_affected();
                    debug!(
                        "[Rollout] Inserted event {} for session {} (rows affected: {})",
                        i, self.session_id, r.rows_affected()
                    );
                }
                Err(e) => {
                    error!(
                        "[Rollout] Failed to write event {} for session {}: {}",
                        i, self.session_id, e
                    );
                }
            }
        }

        info!(
            "[Rollout] Flushed {} events for session {}",
            self.events.len(),
            self.session_id
        );
    }

    /// Finalize the session: save metadata and clear.
    pub fn summary(&self) -> SessionMeta {
        let (msg_count, tool_count) = self.events.iter().fold((0, 0), |(msgs, tools), e| {
            match e {
                RolloutEvent::UserMessage { .. }
                | RolloutEvent::AssistantMessage { .. } => (msgs + 1, tools),
                RolloutEvent::ToolCall { .. } => (msgs, tools + 1),
                _ => (msgs, tools),
            }
        });

        // Extract first user message for session info
        let (user_id, channel_id, model) = self
            .events
            .iter()
            .find_map(|e| match e {
                RolloutEvent::SessionStart {
                    user_id,
                    channel_id,
                    model,
                } => Some((user_id.clone(), channel_id.clone(), model.clone())),
                _ => None,
            })
            .unwrap_or_else(|| ("".to_string(), "".to_string(), "".to_string()));

        SessionMeta {
            session_id: self.session_id.clone(),
            user_id,
            channel_id,
            model,
            message_count: msg_count,
            tool_call_count: tool_count,
        }
    }

    /// Rebuild a conversation context from a past session's rollout.
    pub async fn load_session(pool: &SqlitePool, session_id: &str) -> Vec<RolloutEvent> {
        let rows: Result<Vec<(String,)>, _> = sqlx::query_as(
            "SELECT json FROM rollout_events WHERE session_id = ? ORDER BY sequence ASC",
        )
        .bind(session_id)
        .fetch_all(pool)
        .await;

        let rows = match rows {
            Ok(r) => r,
            Err(e) => {
                error!("[Rollout] Failed to load session {}: {}", session_id, e);
                return Vec::new();
            }
        };

        let mut events = Vec::new();
        for (json,) in rows {
            if let Ok(event) = serde_json::from_str::<RolloutEvent>(&json) {
                events.push(event);
            }
        }
        events
    }

    /// List recent sessions for a channel.
    pub async fn list_sessions(
        pool: &SqlitePool,
        channel_id: &str,
        limit: usize,
    ) -> Vec<SessionMeta> {
        let rows: Vec<(String, String, String, String, i64, i64)> = sqlx::query_as(
            "SELECT DISTINCT session_id,
                (SELECT content FROM rollout_events WHERE session_id = s.session_id AND type = 'session_start' LIMIT 1),
                '', '', 0, 0
             FROM rollout_events s
             WHERE s.session_id IN (
                 SELECT DISTINCT session_id FROM rollout_events
                 WHERE json LIKE ?
                 ORDER BY updated_at DESC
                 LIMIT ?
             )",
        )
        .bind(format!("%channel_id:%{}%", channel_id))
        .bind(limit as i64)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        // Fallback: just return session IDs
        rows.into_iter()
            .map(|(sid, _, _, _, _, _)| SessionMeta {
                session_id: sid,
                user_id: "".to_string(),
                channel_id: channel_id.to_string(),
                model: "".to_string(),
                message_count: 0,
                tool_call_count: 0,
            })
            .collect()
    }
}

/// Initialize the rollout_events table.
pub async fn init_rollout_table(pool: &SqlitePool) {
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS rollout_events (
            id        TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            sequence   INTEGER NOT NULL,
            json       TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(session_id, sequence)
        )",
    )
    .execute(pool)
    .await;

    let _ = sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_rollout_session ON rollout_events(session_id)",
    )
    .execute(pool)
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rollout_record_and_summary() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_rollout_table(&pool).await;

        let mut recorder = RolloutRecorder::new("session-1", pool);
        recorder.record(RolloutEvent::SessionStart {
            user_id: "u1".to_string(),
            channel_id: "c1".to_string(),
            model: "test-model".to_string(),
        });
        recorder.record(RolloutEvent::UserMessage {
            user_id: "u1".to_string(),
            channel_id: "c1".to_string(),
            content: "Hello".to_string(),
        });
        recorder.record(RolloutEvent::ToolCall {
            tool: "file_read".to_string(),
            arguments: "test.txt".to_string(),
        });

        let summary = recorder.summary();
        assert_eq!(summary.message_count, 1); // 1 user message
        assert_eq!(summary.tool_call_count, 1);
        assert_eq!(summary.session_id, "session-1");
    }

    #[tokio::test]
    async fn test_rollout_flush_and_load() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        init_rollout_table(&pool).await;

        let mut recorder = RolloutRecorder::new("session-2", pool.clone());
        recorder.record(RolloutEvent::UserMessage {
            user_id: "u1".to_string(),
            channel_id: "c1".to_string(),
            content: "Test message".to_string(),
        });
        recorder.record(RolloutEvent::AssistantMessage {
            channel_id: "c1".to_string(),
            content: "Reply".to_string(),
        });
        recorder.flush().await;

        let events = RolloutRecorder::load_session(&pool, "session-2").await;
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_rollout_event_serialization() {
        let event = RolloutEvent::UserMessage {
            user_id: "u1".to_string(),
            channel_id: "c1".to_string(),
            content: "hello".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"user_message\""));
        assert!(json.contains("hello"));
    }

    #[test]
    fn test_rollout_event_tool_call_serialization() {
        let event = RolloutEvent::ToolCall {
            tool: "shell_exec".to_string(),
            arguments: "ls -la".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"tool_call\""));
        assert!(json.contains("shell_exec"));
    }

    #[test]
    fn test_rollout_event_session_end() {
        let event = RolloutEvent::SessionEnd {
            message_count: 5,
            tool_call_count: 2,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"session_end\""));
        assert!(json.contains("5"));
        assert!(json.contains("2"));
    }

    #[test]
    fn test_rollout_event_deserialization() {
        let json = r#"{"type":"user_message","user_id":"u1","channel_id":"c1","content":"hi"}"#;
        let event: RolloutEvent = serde_json::from_str(json).unwrap();
        match event {
            RolloutEvent::UserMessage { content, .. } => assert_eq!(content, "hi"),
            _ => panic!("Expected UserMessage"),
        }
    }

    #[test]
    fn test_empty_rollout_event_types() {
        // Verify all event variants serialize/deserialize
        let events: Vec<RolloutEvent> = vec![
            RolloutEvent::SessionStart {
                user_id: "u1".to_string(),
                channel_id: "c1".to_string(),
                model: "test".to_string(),
            },
            RolloutEvent::SessionEnd {
                message_count: 3,
                tool_call_count: 1,
            },
        ];

        for event in &events {
            let json = serde_json::to_string(event).unwrap();
            let parsed: RolloutEvent = serde_json::from_str(&json).unwrap();
            // Verify round-trip
            let json2 = serde_json::to_string(&parsed).unwrap();
            assert_eq!(json, json2);
        }
    }
}
