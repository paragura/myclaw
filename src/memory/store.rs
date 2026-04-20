use sqlx::SqlitePool;
use tracing::{info, debug, error};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Memory {
    pub id: String,
    pub category: String,
    pub content: String,
    pub importance: f32,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ConversationEntry {
    pub id: String,
    pub user_id: String,
    pub channel_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ScheduledTask {
    pub id: String,
    pub name: String,
    pub cron_expression: String,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Clone)]
pub struct MemoryStore {
    pool: SqlitePool,
}

impl MemoryStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    // --- Memory CRUD ---

    pub async fn add_memory(&self, category: &str, content: &str, importance: f32) -> Memory {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        if let Err(e) = sqlx::query(
            "INSERT INTO memories (id, category, content, importance, updated_at) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(category)
        .bind(content)
        .bind(importance)
        .bind(&now)
        .execute(&self.pool)
        .await
        {
            error!("[Memory] Failed to insert memory: {}", e);
            return Memory {
                id,
                category: category.to_string(),
                content: content.to_string(),
                importance,
                created_at: now,
            };
        }

        info!("[Memory] Added: {} (category: {}, importance: {})",
            content.chars().take(50).collect::<String>(), category, importance);

        Memory {
            id: id.clone(),
            category: category.to_string(),
            content: content.to_string(),
            importance,
            created_at: now.clone(),
        }
    }

    pub async fn get_memories(&self, category: Option<&str>, limit: usize) -> Vec<Memory> {
        let memories = match category {
            Some(cat) => sqlx::query_as(
                "SELECT id, category, content, importance, created_at FROM memories WHERE category = ? ORDER BY updated_at DESC LIMIT ?"
            )
            .bind(cat)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await,
            None => sqlx::query_as(
                "SELECT id, category, content, importance, created_at FROM memories ORDER BY updated_at DESC LIMIT ?"
            )
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await,
        };

        match memories {
            Ok(rows) => {
                debug!("[Memory] Retrieved {} memories", rows.len());
                rows
            }
            Err(e) => {
                debug!("[Memory] Error retrieving memories: {}", e);
                Vec::new()
            }
        }
    }

    pub async fn search_memories(&self, query: &str, limit: usize) -> Vec<Memory> {
        let pattern = format!("%{}%", query);
        let memories = sqlx::query_as(
            "SELECT id, category, content, importance, created_at FROM memories WHERE content LIKE ? ORDER BY importance DESC LIMIT ?"
        )
        .bind(&pattern)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await;

        match memories {
            Ok(rows) => rows,
            Err(e) => {
                debug!("[Memory] Search error: {}", e);
                Vec::new()
            }
        }
    }

    pub async fn delete_memory(&self, id: &str) -> bool {
        let result = sqlx::query("DELETE FROM memories WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await;

        match result {
            Ok(r) => r.rows_affected() > 0,
            Err(_) => false,
        }
    }

    // --- Conversation History ---

    pub async fn add_conversation(&self, user_id: &str, channel_id: &str, role: &str, content: &str) {
        let id = Uuid::new_v4().to_string();
        if let Err(e) = sqlx::query(
            "INSERT INTO conversations (id, user_id, channel_id, role, content) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(user_id)
        .bind(channel_id)
        .bind(role)
        .bind(content)
        .execute(&self.pool)
        .await
        {
            error!("[Memory] Failed to insert conversation: {}", e);
            return;
        }

        // Keep last 50 messages per channel for context
        self.prune_conversations(channel_id, 50).await;
    }

    pub async fn get_conversation_history(&self, channel_id: &str, limit: usize) -> Vec<ConversationEntry> {
        sqlx::query_as(
            "SELECT id, user_id, channel_id, role, content, created_at FROM conversations WHERE channel_id = ? ORDER BY created_at DESC LIMIT ?"
        )
        .bind(channel_id)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
    }

    async fn prune_conversations(&self, channel_id: &str, limit: usize) {
        let _ = sqlx::query(
            "DELETE FROM conversations WHERE rowid NOT IN (
                SELECT rowid FROM conversations WHERE channel_id = ? ORDER BY created_at DESC LIMIT ?
            )"
        )
        .bind(channel_id)
        .bind(limit as i64)
        .execute(&self.pool)
        .await;
    }

    // --- Scheduled Tasks ---

    pub async fn add_scheduled_task(&self, name: &str, cron_expression: &str) -> ScheduledTask {
        let id = Uuid::new_v4().to_string();
        let next_run = Self::calculate_next_run(cron_expression);
        let now = chrono::Utc::now().to_rfc3339();

        if let Err(e) = sqlx::query(
            "INSERT INTO scheduled_tasks (id, name, cron_expression, next_run) VALUES (?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(name)
        .bind(cron_expression)
        .bind(&next_run)
        .execute(&self.pool)
        .await
        {
            error!("[Scheduler] Failed to insert scheduled task: {}", e);
            return ScheduledTask {
                id,
                name: name.to_string(),
                cron_expression: cron_expression.to_string(),
                last_run: None,
                next_run: Some(next_run),
                enabled: true,
                created_at: now,
            };
        }

        ScheduledTask {
            id,
            name: name.to_string(),
            cron_expression: cron_expression.to_string(),
            last_run: None,
            next_run: Some(next_run),
            enabled: true,
            created_at: now,
        }
    }

    pub async fn get_all_tasks(&self) -> Vec<ScheduledTask> {
        sqlx::query_as(
            "SELECT id, name, cron_expression, last_run, next_run, enabled, created_at FROM scheduled_tasks"
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
    }

    pub async fn update_task_run(&self, id: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        let next_run = self.get_task_next_run(id).await;

        sqlx::query(
            "UPDATE scheduled_tasks SET last_run = ?, next_run = ?, updated_at = ? WHERE id = ?"
        )
        .bind(&now)
        .bind(&next_run)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await
        .ok();
    }

    async fn get_task_next_run(&self, id: &str) -> Option<String> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT cron_expression FROM scheduled_tasks WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .ok()?;

        let cron_expr = row?.0;
        Some(Self::calculate_next_run(&cron_expr))
    }

    fn calculate_next_run(_cron_expr: &str) -> String {
        // Simple: calculate next run in 1 hour
        // In production, use the cron crate for proper parsing
        (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339()
    }

    // --- Channel Config ---

    pub async fn get_always_respond_channels(&self) -> Vec<String> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT channel_id FROM channel_config WHERE config_key = 'always_respond' AND config_value = 1"
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        rows.into_iter().map(|r| r.0).collect()
    }

    pub async fn add_always_respond_channel(&self, channel_id: &str) -> bool {
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query(
            "INSERT OR REPLACE INTO channel_config (channel_id, config_key, config_value, updated_at) VALUES (?, 'always_respond', 1, ?)"
        )
        .bind(channel_id)
        .bind(&now)
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => {
                info!("[Channel] Added always-respond channel: {}", channel_id);
                true
            }
            Err(_) => false,
        }
    }

    pub async fn remove_always_respond_channel(&self, channel_id: &str) -> bool {
        let result = sqlx::query(
            "DELETE FROM channel_config WHERE channel_id = ? AND config_key = 'always_respond'"
        )
        .bind(channel_id)
        .execute(&self.pool)
        .await;

        match result {
            Ok(r) => {
                let deleted = r.rows_affected() > 0;
                if deleted {
                    info!("[Channel] Removed always-respond channel: {}", channel_id);
                }
                deleted
            }
            Err(_) => false,
        }
    }

    pub async fn is_channel_always_respond(&self, channel_id: &str) -> bool {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM channel_config WHERE channel_id = ? AND config_key = 'always_respond' AND config_value = 1"
        )
        .bind(channel_id)
        .fetch_optional(&self.pool)
        .await
        .unwrap_or(None);

        row.map(|r| r.0 > 0).unwrap_or(false)
    }

    pub async fn list_channel_configs(&self) -> Vec<(String, String, bool)> {
        let rows: Vec<(String, String, i64)> = sqlx::query_as(
            "SELECT channel_id, config_key, config_value FROM channel_config WHERE config_key = 'always_respond'"
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        rows.into_iter()
            .map(|r| (r.0, r.1, r.2 > 0))
            .collect()
    }

    // --- Coding Tasks ---

    pub async fn add_coding_task(
        &self,
        user_id: &str,
        channel_id: &str,
        description: &str,
    ) -> (String, String) {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        if let Err(e) = sqlx::query(
            "INSERT INTO coding_tasks (id, user_id, channel_id, description, status, created_at) VALUES (?, ?, ?, ?, 'pending', ?)"
        )
        .bind(&id)
        .bind(user_id)
        .bind(channel_id)
        .bind(description)
        .bind(&now)
        .execute(&self.pool)
        .await
        {
            error!("[Coding] Failed to insert coding task: {}", e);
            return (id, now.clone());
        }

        (id, now)
    }

    pub async fn update_coding_task_result(
        &self,
        id: &str,
        code: Option<&str>,
        status: &str,
        result: Option<&str>,
    ) {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE coding_tasks SET code = ?, status = ?, result = ?, completed_at = ?, updated_at = ? WHERE id = ?"
        )
        .bind(code)
        .bind(status)
        .bind(result)
        .bind(if status == "completed" { Some(&now) } else { None })
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await
        .ok();
    }
}
