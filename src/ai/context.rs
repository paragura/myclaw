use crate::ai::client::ChatMessage;
use crate::memory::store::MemoryStore;
use std::sync::Arc;
use tracing::debug;

#[derive(Clone)]
pub struct ContextManager {
    store: Arc<MemoryStore>,
    system_prompt: String,
}

impl ContextManager {
    pub fn new(store: Arc<MemoryStore>, system_prompt: String) -> Self {
        Self {
            store,
            system_prompt,
        }
    }

    pub fn get_system_prompt(&self) -> &str {
        &self.system_prompt
    }

    pub async fn get_messages_for_channel(
        &self,
        channel_id: &str,
        max_messages: usize,
    ) -> Vec<ChatMessage> {
        let mut messages = vec![ChatMessage {
            role: "system".to_string(),
            content: self.system_prompt.clone(),
        }];

        // Get relevant memories
        let memories = self.store.get_memories(None, 10).await;
        if !memories.is_empty() {
            let memory_content: Vec<String> = memories
                .iter()
                .map(|m| format!("[{}]: {}", m.category, m.content))
                .collect();
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: format!("Known memories:\n{}", memory_content.join("\n")),
            });
        }

        // Get conversation history
        let history = self.store.get_conversation_history(channel_id, max_messages).await;
        for entry in history.iter().rev() {
            messages.push(ChatMessage {
                role: entry.role.clone(),
                content: entry.content.clone(),
            });
        }

        debug!("Built context with {} messages for channel {}", messages.len(), channel_id);
        messages
    }

    pub async fn add_to_context(&self, user_id: &str, channel_id: &str, role: &str, content: &str) {
        self.store
            .add_conversation(user_id, channel_id, role, content)
            .await;
    }

    pub async fn save_learning(&self, category: &str, content: &str, importance: f32) {
        self.store.add_memory(category, content, importance).await;
    }
}
