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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_initializing_display() {
        assert_eq!(format!("{}", AgentStatus::Initializing), "初期化中");
    }

    #[test]
    fn test_status_running_display() {
        assert_eq!(format!("{}", AgentStatus::Running), "実行中");
    }

    #[test]
    fn test_status_thinking_display() {
        assert_eq!(format!("{}", AgentStatus::Thinking), "思考中");
    }

    #[test]
    fn test_status_interrupted_display() {
        assert_eq!(format!("{}", AgentStatus::Interrupted), "中断済み");
    }

    #[test]
    fn test_status_using_skill_display() {
        let status = AgentStatus::UsingSkill {
            skill: "shell_exec".to_string(),
        };
        assert_eq!(format!("{}", status), "スキル使用中: shell_exec");
    }

    #[test]
    fn test_status_completed_display_truncates_long_result() {
        let long_result = "a".repeat(200);
        let status = AgentStatus::Completed { result: long_result };
        let display = format!("{}", status);
        assert!(display.starts_with("完了: "));
        // The Display impl takes .chars().take(50) of the result
        // "完了: " prefix is 4 bytes + "..." suffix = 4 chars + 50 char 'a's
        // So after prefix, we should have exactly 50 'a' chars
        let result_part = display.strip_prefix("完了: ").unwrap();
        assert_eq!(result_part.chars().count(), 50);
        assert!(result_part.chars().all(|c| c == 'a'));
    }

    #[test]
    fn test_status_failed_display() {
        let status = AgentStatus::Failed {
            error: "connection timeout".to_string(),
        };
        assert_eq!(format!("{}", status), "失敗: connection timeout");
    }

    #[test]
    fn test_status_equality() {
        assert_eq!(AgentStatus::Initializing, AgentStatus::Initializing);
        assert_eq!(
            AgentStatus::Completed { result: "done".to_string() },
            AgentStatus::Completed { result: "done".to_string() },
        );
        assert_ne!(AgentStatus::Running, AgentStatus::Thinking);
    }

    #[test]
    fn test_status_serialization() {
        let status = AgentStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"Running\"");

        let status = AgentStatus::Completed { result: "done".to_string() };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("Completed"));
        assert!(json.contains("done"));
    }

    #[test]
    fn test_status_deserialization() {
        let status: AgentStatus = serde_json::from_str("\"Initializing\"").unwrap();
        assert_eq!(status, AgentStatus::Initializing);

        let status: AgentStatus = serde_json::from_str("\"Interrupted\"").unwrap();
        assert_eq!(status, AgentStatus::Interrupted);
    }

    #[test]
    fn test_status_clone_and_debug() {
        let status = AgentStatus::UsingSkill { skill: "test".to_string() };
        let cloned = status.clone();
        assert_eq!(status, cloned);

        let debug_str = format!("{:?}", AgentStatus::Running);
        assert!(debug_str.contains("Running"));
    }
}
