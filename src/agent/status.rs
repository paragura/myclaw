use serde::{Deserialize, Serialize};

/// Status of an agent, inspired by Codex's AgentStatus
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentStatus {
    /// Agent is being initialized
    Initializing,
    /// Agent is actively working
    Running,
    /// Agent is thinking/reasoning
    Thinking,
    /// Agent is using a skill/tool
    UsingSkill { skill: String },
    /// Agent has completed successfully
    Completed { result: String },
    /// Agent failed
    Failed { error: String },
    /// Agent was interrupted
    Interrupted,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Initializing => write!(f, "初期化中"),
            AgentStatus::Running => write!(f, "実行中"),
            AgentStatus::Thinking => write!(f, "思考中"),
            AgentStatus::UsingSkill { skill } => write!(f, "スキル使用中: {}", skill),
            AgentStatus::Completed { result } => write!(f, "完了: {}", result.chars().take(50).collect::<String>()),
            AgentStatus::Failed { error } => write!(f, "失敗: {}", error),
            AgentStatus::Interrupted => write!(f, "中断済み"),
        }
    }
}
