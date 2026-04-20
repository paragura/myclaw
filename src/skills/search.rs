use std::sync::Arc;

use crate::memory::store::MemoryStore;

#[derive(Clone)]
pub struct SearchSkill;

impl SearchSkill {
    pub fn new() -> Self {
        Self
    }

    pub async fn search_memories(&self, query: &str, store: &Arc<MemoryStore>) -> String {
        if query.is_empty() {
            return "検索キーワードを入力してください: `!skill search_memories <キーワード>`".to_string();
        }

        let memories = store.search_memories(query, 20).await;

        if memories.is_empty() {
            return format!("`{}` の検索結果はありませんでした。", query);
        }

        let results: Vec<String> = memories
            .iter()
            .map(|m| {
                format!(
                    "  `[{}]` [{}] {}\n      重要度: {:.1}",
                    m.id.chars().take(8).collect::<String>(),
                    m.category,
                    m.content.chars().take(100).collect::<String>(),
                    m.importance
                )
            })
            .collect();

        format!(
            "🔍 **`{}` の検索結果** ({}件)\n\n{}",
            query,
            results.len(),
            results.join("\n")
        )
    }
}
