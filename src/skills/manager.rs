use serde::{Deserialize, Serialize};
use tracing::{info, error};
use std::sync::Arc;
use std::collections::HashMap;
use std::fs;

use crate::ai::client::{AIClient, ChatMessage, ToolDefinition};
use crate::ai::stream::{StreamClient, StreamItem};
use crate::memory::store::MemoryStore;
use super::file_ops::FileOpSkill;
use super::search::SearchSkill;
use super::system_info::SystemInfoSkill;
use crate::tools::shell_exec::ShellExecTool;
use crate::tools::file_read::FileReadTool;
use crate::tools::file_write::FileWriteTool;
use crate::tools::web_fetch::WebFetchTool;

/// Represents a skill that can be invoked
#[derive(Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub func: Arc<dyn Fn(&str, Arc<MemoryStore>) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>> + Send + Sync>,
}

impl Skill {
    pub fn new(
        name: &str,
        description: &str,
        func: impl Fn(&str, Arc<MemoryStore>) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>> + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            func: Arc::new(func),
        }
    }

    pub async fn invoke(&self, args: &str, store: Arc<MemoryStore>) -> String {
        (self.func)(args, store).await
    }
}

/// Result of executing a skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResult {
    pub skill: String,
    pub output: String,
    pub success: bool,
}

/// Manager that holds and dispatches skills
pub struct SkillsManager {
    skills: HashMap<String, Skill>,
}

impl SkillsManager {
    pub fn new() -> Self {
        let mut mgr = Self {
            skills: HashMap::new(),
        };
        mgr.register_builtins();
        mgr
    }

    fn register_builtins(&mut self) {
        // File read tool
        let file_skill_read = FileReadTool::new();
        self.register(Skill {
            name: "file_read".to_string(),
            description: "Read file contents. Args: <filepath>".to_string(),
            func: Arc::new(move |args: &str, _store: Arc<MemoryStore>| {
                let tool = file_skill_read.clone();
                let args = args.to_string();
                Box::pin(async move { tool.read(&args) })
            }),
        });

        let file_skill_list = FileOpSkill::new();
        self.register(Skill {
            name: "file_list".to_string(),
            description: "List directory contents. Args: <directory_path>".to_string(),
            func: Arc::new(move |args: &str, store: Arc<MemoryStore>| {
                let skill = file_skill_list.clone();
                let args = args.to_string();
                Box::pin(async move { skill.list_dir(&args, &store).await })
            }),
        });

        // Search skill
        let search_skill = SearchSkill::new();
        self.register(Skill::new(
            "search_memories",
            "Search stored memories. Args: <query>",
            move |args: &str, store: Arc<MemoryStore>| {
                let skill = search_skill.clone();
                let q = args.trim().to_string();
                Box::pin(async move { skill.search_memories(&q, &store).await })
            },
        ));

        // System info skill
        let sys_skill_info = SystemInfoSkill::new();
        self.register(Skill::new(
            "sys_info",
            "Get system information. Args: (optional) <info_type>",
            move |args: &str, _store: Arc<MemoryStore>| {
                let skill = sys_skill_info.clone();
                let args = args.to_string();
                Box::pin(async move { skill.get_info(&args).await })
            },
        ));

        let sys_skill_proc = SystemInfoSkill::new();
        self.register(Skill::new(
            "sys_process",
            "List running processes. Args: (optional) filter",
            move |args: &str, _store: Arc<MemoryStore>| {
                let skill = sys_skill_proc.clone();
                let args = args.to_string();
                Box::pin(async move { skill.list_processes(&args).await })
            },
        ));

        // Shell exec tool
        let shell_tool = ShellExecTool::new();
        self.register(Skill {
            name: "shell_exec".to_string(),
            description: "Execute a shell command. Args: <command> (e.g. 'ls -la', 'curl -s https://...')".to_string(),
            func: Arc::new(move |args: &str, _store: Arc<MemoryStore>| {
                let tool = shell_tool.clone();
                let args = args.to_string();
                Box::pin(async move { tool.execute(&args) })
            }),
        });

        // File write tool
        let fw_tool = FileWriteTool::new();
        self.register(Skill {
            name: "file_write".to_string(),
            description: "Write content to a file. Args: <filepath> <content>".to_string(),
            func: Arc::new(move |args: &str, _store: Arc<MemoryStore>| {
                let tool = fw_tool.clone();
                let args = args.to_string();
                Box::pin(async move { tool.write(&args) })
            }),
        });

        // Web fetch tool
        let wf_tool = WebFetchTool::new();
        self.register(Skill {
            name: "web_fetch".to_string(),
            description: "Fetch a web page and return title + content. Args: <url>".to_string(),
            func: Arc::new(move |args: &str, _store: Arc<MemoryStore>| {
                let tool = wf_tool.clone();
                let url = args.to_string();
                Box::pin(async move {
                    tokio::spawn(async move { tool.fetch(&url).await })
                        .await
                        .unwrap_or_else(|e| format!("Web fetch error: {}", e))
                })
            }),
        });

        info!("[SkillsManager] Registered {} built-in skills", self.skills.len());
    }

    pub fn register(&mut self, skill: Skill) {
        info!("[SkillsManager] Registered skill: {}", skill.name);
        self.skills.insert(skill.name.clone(), skill);
    }

    pub async fn invoke(&self, name: &str, args: &str, store: &Arc<MemoryStore>) -> String {
        match self.skills.get(name) {
            Some(skill) => skill.invoke(args, (*store).clone()).await,
            None => format!("Unknown skill: `{}`. Use `!skills` to list available skills.", name),
        }
    }

    pub fn list(&self) -> Vec<&Skill> {
        self.skills.values().collect()
    }

    pub fn list_clone(&self) -> Vec<Skill> {
        self.skills.values().cloned().collect()
    }

    pub fn list_descriptions(&self) -> String {
        let mut lines: Vec<String> = Vec::new();

        // Rust tools
        lines.push("  **Rust Tools:**".to_string());
        for s in self.skills.values() {
            lines.push(format!("  `!skill {}` - {}", s.name, s.description));
        }

        // Markdown skills
        let md_skills = SkillsManager::load_markdown_skills();
        if !md_skills.is_empty() {
            lines.push("".to_string());
            lines.push("  **Markdown Skills:**".to_string());
            for s in &md_skills {
                lines.push(format!("  `SKILL.md` {} - {}", s.name, s.description));
            }
        }

        lines.join("\n")
    }

    /// Convert skills to OpenAI-compatible tool definitions.
    pub fn to_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.skills.values().map(|skill| {
            ToolDefinition {
                r#type: "function".to_string(),
                function: crate::ai::client::FunctionDefinition {
                    name: skill.name.clone(),
                    description: skill.description.clone(),
                    parameters: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "command": {
                                "type": "string",
                                "description": "The arguments for the skill"
                            }
                        },
                        "required": ["command"]
                    }),
                },
            }
        }).collect()
    }

    /// Load markdown skills from src/skills/markdown/*/SKILL.md
    pub fn load_markdown_skills() -> Vec<MarkdownSkill> {
        let mut skills = Vec::new();
        let base = "src/skills/markdown";

        if let Ok(entries) = fs::read_dir(base) {
            for entry in entries.filter_map(|e| e.ok()) {
                let skill_path = entry.path().join("SKILL.md");
                if skill_path.is_file() {
                    if let Ok(content) = fs::read_to_string(&skill_path) {
                        if let Some(skill) = MarkdownSkill::from_markdown(&content) {
                            skills.push(skill);
                        }
                    }
                }
            }
        }

        skills.sort_by(|a, b| a.name.cmp(&b.name));
        skills
    }
}

/// A skill defined as a markdown file (SKILL.md format).
#[derive(Debug, Clone)]
pub struct MarkdownSkill {
    pub name: String,
    pub description: String,
}

impl MarkdownSkill {
    /// Parse frontmatter from a SKILL.md file.
    fn from_markdown(content: &str) -> Option<Self> {
        let content = content.trim();
        if !content.starts_with("---") {
            return None;
        }

        let end = content[3..].find("-->")?;
        let frontmatter = &content[3..3 + end];

        let name = frontmatter
            .lines()
            .find(|l| l.starts_with("name:"))
            .and_then(|l| l.split(':').nth(1).map(|s| s.trim().to_string()))?;

        let description = frontmatter
            .lines()
            .find(|l| l.starts_with("description:"))
            .and_then(|l| l.split(':').nth(1).map(|s| s.trim().to_string()))?;

        Some(Self { name, description })
    }
}

/// Execute a complex task with thinking display.
/// Streams AI response and shows reasoning steps as Discord messages.
pub async fn execute_with_thinking(
    stream_client: &StreamClient,
    _ai_client: &Arc<AIClient>,
    messages: &[ChatMessage],
    thinking_sender: impl Fn(&str) + Send + Sync + 'static,
    final_sender: impl Fn(&str) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync + 'static,
    skills_mgr: &SkillsManager,
    store: &Arc<MemoryStore>,
) -> String {
    // First, check if the request involves using a skill
    let skill_decision = decide_skill_usage(stream_client, messages, skills_mgr).await;

    if let Some(skill_name) = skill_decision {
        // Extract args from the last user message
        let last_user_msg = messages.iter().rev().find(|m| m.role == "user");
        let args = last_user_msg.map(|m| m.content.as_str()).unwrap_or("");

        // Show thinking about skill usage
        thinking_sender(&format!("🔧 **スキル `{}` を実行中...**", skill_name));

        let output = skills_mgr.invoke(&skill_name, args, store).await;

        thinking_sender("✅ スキル実行完了");
        return output;
    }

    // Stream the response with thinking display
    let items = stream_client.stream_chat(messages).await;
    let mut reasoning_parts = Vec::new();
    let mut content_parts = Vec::new();
    let mut last_reasoning_len = 0;

    for item in items {
        match item {
            StreamItem::Reasoning { content, done } => {
                if content.len() > last_reasoning_len {
                    let new_content = &content[last_reasoning_len..];
                    if !new_content.trim().is_empty() {
                        let display = format!("🤔 {}", new_content.trim().chars().take(200).collect::<String>());
                        thinking_sender(&display);
                    }
                    last_reasoning_len = content.len();
                }
                reasoning_parts.push(content);
                if done {
                    thinking_sender("💭 思考完了");
                }
            }
            StreamItem::Content(part) => {
                content_parts.push(part);
            }
            StreamItem::ToolCall { .. } => {
                // Tool calls are handled by the tool-use loop in handle_free_chat_thinking
            }
            StreamItem::Error(e) => {
                error!("[Thinking] Stream error: {}", e);
                thinking_sender(&format!("❌ エラー: {}", e));
                return format!("エラーが発生しました: {}", e);
            }
        }
    }

    let final_answer = content_parts.join("");
    if final_answer.is_empty() && !reasoning_parts.is_empty() {
        // If no content, use reasoning as the answer
        let answer = reasoning_parts.join("\n");
        final_sender(&answer).await;
        answer
    } else {
        let answer = final_answer.clone();
        final_sender(&answer).await;
        answer
    }
}

/// Ask the AI to decide if a skill should be used
async fn decide_skill_usage(
    stream_client: &StreamClient,
    messages: &[ChatMessage],
    skills_mgr: &SkillsManager,
) -> Option<String> {
    let skill_list = skills_mgr.list_descriptions();
    let decision_messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: format!(
                "あなたはスキル使用を判断するアシスタントです。\n\
                以下のスキルが利用可能です:\n{}\n\
                使用するスキルがあればその名前だけを返してください。\n\
                不要であれば空文字列を返してください。\n\
                日本語で考えてください。",
                skill_list
            ),
            tool_calls: None,
            tool_call_id: None,
        },
        ChatMessage {
            role: "user".to_string(),
            content: messages.last().map(|m| m.content.clone()).unwrap_or_default(),
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    // Quick non-streaming call for skill decision
    match stream_client.chat(&decision_messages).await {
        Ok(response) => {
            let lower = response.to_lowercase();
            let all_skills: Vec<String> = skills_mgr.list_clone().iter().map(|s| s.name.clone()).collect();
            for skill in &all_skills {
                if lower.contains(skill) {
                    return Some(skill.clone());
                }
            }
            None
        }
        Err(_) => None,
    }
}
