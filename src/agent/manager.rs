use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, debug, error, warn};
use uuid::Uuid;

use crate::ai::client::{AIClient, ChatMessage};
use crate::ai::stream::StreamClient;
use crate::memory::store::MemoryStore;
use crate::skills::manager::SkillsManager;
use super::status::AgentStatus;

/// A spawned agent that handles a specific task
#[derive(Clone)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub status: AgentStatus,
    pub created_at: Instant,
    pub result: Option<String>,
}

/// Manager for spawning and tracking sub-agents
/// Inspired by Codex's AgentControl pattern
pub struct AgentManager {
    agents: std::sync::Arc<tokio::sync::Mutex<HashMap<String, Agent>>>,
    handles: std::sync::Arc<tokio::sync::Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    max_concurrent: usize,
}

impl AgentManager {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            agents: std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            handles: std::sync::Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            max_concurrent,
        }
    }

    /// Get the number of currently active agents
    pub async fn active_count(&self) -> usize {
        self.agents.lock().await.iter()
            .filter(|(_, a)| matches!(a.status, AgentStatus::Running | AgentStatus::Thinking | AgentStatus::UsingSkill { .. }))
            .count()
    }

    /// Check if we can spawn a new agent
    pub async fn can_spawn(&self) -> bool {
        self.active_count().await < self.max_concurrent
    }

    /// Spawn a new agent to handle a complex task
    /// Returns the agent ID immediately
    pub async fn spawn_agent(
        &self,
        name: &str,
        prompt: &str,
        ai_client: Arc<AIClient>,
        stream_client: Arc<StreamClient>,
        store: Arc<MemoryStore>,
        skills_mgr: Arc<SkillsManager>,
        thinking_sender: impl Fn(&str) + Send + Sync + 'static,
        status_updater: impl Fn(&str, AgentStatus) + Send + Sync + 'static,
    ) -> String {
        let id = Uuid::new_v4().to_string();

        // Register agent
        {
            let mut agents = self.agents.lock().await;
            agents.insert(id.clone(), Agent {
                id: id.clone(),
                name: name.to_string(),
                status: AgentStatus::Initializing,
                created_at: Instant::now(),
                result: None,
            });
        }

        status_updater(&id, AgentStatus::Initializing);
        thinking_sender(&format!("🧠 **エージェント `{}` を起動しました**", name));

        // Spawn the agent task
        let agent_id = id.clone();
        let agent_name = name.to_string();
        let prompt = prompt.to_string();
        let prompt_display = prompt.clone();
        let agents = self.agents.clone();
        let handles = self.handles.clone();
        let ai_client_clone = ai_client.clone();
        let stream_client_clone = stream_client.clone();
        let store_clone = store.clone();
        let skills_clone = skills_mgr.clone();

        let handles_for_spawn = handles.clone();
        let handle = tokio::spawn(async move {
            // Update status to running
            {
                let mut agents = agents.lock().await;
                if let Some(agent) = agents.get_mut(&agent_id) {
                    agent.status = AgentStatus::Running;
                }
            }
            status_updater(&agent_id, AgentStatus::Running);
            thinking_sender("⚡ エージェント実行開始");

            // Build context messages
            let memories = store_clone.get_memories(None, 5).await;
            let memory_context = if !memories.is_empty() {
                let texts: Vec<String> = memories.iter()
                    .map(|m| format!("[{}]: {}", m.category, m.content))
                    .collect();
                format!("Known memories:\n{}", texts.join("\n"))
            } else {
                String::new()
            };

            let messages = vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: format!(
                        "あなたはmyclawのサブエージェント「{}」です。\n\
                        与えられたタスクを完了してください。\n\
                        スキルが利用可能な場合はそれを使ってください。\n\
                        \n\
                        {}",
                        agent_name, memory_context
                    ),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                },
            ];

            // Try streaming first, fall back to non-streaming
            let result = Self::execute_agent_task(
                &agent_id,
                &agents,
                &thinking_sender,
                &stream_client_clone,
                &ai_client_clone,
                &messages,
                &skills_clone,
                &store_clone,
            ).await;

            // Update agent status
            {
                let mut agents = agents.lock().await;
                if let Some(agent) = agents.get_mut(&agent_id) {
                    agent.result = Some(result.clone());
                    agent.status = AgentStatus::Completed { result: result.clone() };
                }
            }
            status_updater(&agent_id, AgentStatus::Completed { result });

            // Cleanup completed agent after a delay (keep for 5 min)
            let cleanup_id = agent_id.clone();
            let cleanup_agents = agents.clone();
            let cleanup_handles = handles_for_spawn.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(300)).await;
                let mut agents = cleanup_agents.lock().await;
                agents.remove(&cleanup_id);
                let mut h = cleanup_handles.lock().await;
                h.remove(&cleanup_id);
                debug!("[AgentManager] Cleaned up agent {}", cleanup_id);
            });
        });

        // Store the join handle for potential abort
        {
            let mut h = handles.lock().await;
            h.insert(id.clone(), handle);
        }
        drop(handles);

        info!("[AgentManager] Spawned agent {}: {} (id: {})", name, prompt_display.chars().take(50).collect::<String>(), id);
        id
    }

    async fn execute_agent_task(
        agent_id: &str,
        _agents: &tokio::sync::Mutex<HashMap<String, Agent>>,
        thinking_sender: &(impl Fn(&str) + Sync),
        stream_client: &StreamClient,
        ai_client: &AIClient,
        messages: &[ChatMessage],
        _skills_mgr: &SkillsManager,
        _store: &MemoryStore,
    ) -> String {
        use crate::ai::stream::StreamItem;

        let items = stream_client.stream_chat(messages).await;
        let mut content_parts = Vec::new();

        for item in items {
            match item {
                StreamItem::Reasoning { content, done } => {
                    if !content.is_empty() {
                        let display = format!("💭 {}", content.chars().take(150).collect::<String>());
                        thinking_sender(&display);
                    }
                    if done {
                        thinking_sender("思考完了");
                    }
                }
                StreamItem::Content(part) => {
                    content_parts.push(part);
                }
                StreamItem::Error(e) => {
                    error!("[Agent {}] Stream error: {}", agent_id, e);
                    return format!("エラー: {}", e);
                }
            }
        }

        let answer = content_parts.join("");
        if answer.is_empty() {
            // Fall back to non-streaming
            warn!("[Agent {}] Stream returned empty, falling back to non-streaming", agent_id);
            thinking_sender("⚡ 通常モードで再実行中");
            match ai_client.chat(messages).await {
                Ok(resp) => resp,
                Err(e) => format!("AIエラー: {}", e),
            }
        } else {
            answer
        }
    }

    /// Get status of a specific agent
    pub async fn get_status(&self, agent_id: &str) -> Option<AgentStatus> {
        self.agents.lock().await.get(agent_id).map(|a| a.status.clone())
    }

    /// List all agents
    pub async fn list_agents(&self) -> Vec<Agent> {
        self.agents.lock().await.values().cloned().collect()
    }

    /// Interrupt an agent
    pub async fn interrupt_agent(&self, agent_id: &str) -> bool {
        // Try to abort the running task first
        {
            let mut handles = self.handles.lock().await;
            if let Some(handle) = handles.remove(agent_id) {
                handle.abort();
                info!("[AgentManager] Aborted agent task {}", agent_id);
            }
        }
        // Then update status
        let mut agents = self.agents.lock().await;
        if let Some(agent) = agents.get_mut(agent_id) {
            if matches!(agent.status, AgentStatus::Running | AgentStatus::Thinking | AgentStatus::Initializing) {
                agent.status = AgentStatus::Interrupted;
                info!("[AgentManager] Interrupted agent {}", agent_id);
                return true;
            }
        }
        false
    }
}
