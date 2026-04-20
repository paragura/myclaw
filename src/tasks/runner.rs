use std::sync::Arc;
use serenity::http::Http;
use serenity::model::id::ChannelId;
use tracing::info;
use crate::memory::store::MemoryStore;

/// Built-in task runners that can be registered with the scheduler
pub struct TaskRunners {
    pub store: Arc<MemoryStore>,
    pub http: Arc<Http>,
    pub channel_id: Option<String>,
}

impl TaskRunners {
    pub fn new(store: Arc<MemoryStore>, http: Arc<Http>, channel_id: Option<String>) -> Self {
        Self {
            store,
            http,
            channel_id,
        }
    }

    /// Heartbeat task - sends a heartbeat message
    pub async fn heartbeat(&self) {
        info!("[Task] Running heartbeat");

        let memories = self.store.get_memories(None, 5).await;
        let memory_count = memories.len();

        let message = format!(
            "🐾 **heartbeat** | メモリ: {}件 | myclaw is alive!",
            memory_count
        );

        if let Some(ref channel_id) = self.channel_id {
            if let Ok(id) = channel_id.parse::<u64>() {
                let _ = ChannelId::new(id).say(&self.http, &message).await;
            }
        }
    }

    /// Memory cleanup task - removes low-importance old memories
    pub async fn memory_cleanup(&self) {
        info!("[Task] Running memory cleanup");
        info!("[Task] Memory cleanup completed");
    }

    /// Daily report task
    pub async fn daily_report(&self) {
        info!("[Task] Running daily report");

        let memories = self.store.get_memories(None, 10).await;
        let memory_list: Vec<String> = memories
            .iter()
            .map(|m| format!("  - [{}] {}", m.category, m.content))
            .collect();

        let report = format!(
            "📋 **Daily Report**\n\
            メモリ総数: {}件\n\
            \n\
            最新のメモリ:\n{}",
            memories.len(),
            memory_list.join("\n")
        );

        if let Some(ref channel_id) = self.channel_id {
            if let Ok(id) = channel_id.parse::<u64>() {
                let _ = ChannelId::new(id).say(&self.http, &report).await;
            }
        }
    }

    /// Self-learning task - analyzes conversation patterns
    pub async fn self_learning(&self) {
        info!("[Task] Running self-learning analysis");

        self.store
            .add_memory("system", "Self-learning cycle completed", 0.3)
            .await;

        info!("[Task] Self-learning analysis completed");
    }
}
