use std::sync::Arc;
use crate::ai::client::AIClient;
use crate::memory::store::MemoryStore;
use tracing::info;

#[derive(Debug, Clone)]
pub struct CodeTask {
    pub id: String,
    pub description: String,
    pub code: String,
    pub language: String,
    pub status: CodeTaskStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CodeTaskStatus {
    Pending,
    Generating,
    Completed,
    Failed,
}

impl std::fmt::Display for CodeTaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodeTaskStatus::Pending => write!(f, "pending"),
            CodeTaskStatus::Generating => write!(f, "generating"),
            CodeTaskStatus::Completed => write!(f, "completed"),
            CodeTaskStatus::Failed => write!(f, "failed"),
        }
    }
}

#[derive(Clone)]
pub struct CodingExecutor {
    ai_client: Arc<AIClient>,
    store: Arc<MemoryStore>,
    system_prompt: String,
}

impl CodingExecutor {
    pub fn new(ai_client: Arc<AIClient>, store: Arc<MemoryStore>) -> Self {
        let system_prompt = r#"あなたは熟練したRustプログラマーです。
ユーザーの要件に基づいて、安全で効率的なコードを生成してください。

以下の点を満たすコードを生成してください:
1. エラーハンドリングを適切に行う
2. 日本語のコメントを付ける
3. 実行可能でテスト済みのコードを生成する
4. 必要に応じて依存関係も記載する

コードブロックで囲んで出力してください。"#.to_string();

        Self {
            ai_client,
            store,
            system_prompt,
        }
    }

    pub fn get_system_prompt(&self) -> &str {
        &self.system_prompt
    }

    pub async fn generate_code(&self, description: &str, language: &str) -> Result<String, Box<dyn std::error::Error>> {
        info!("[Coding] Generating code: {} (language: {})", description, language);

        let messages = vec![
            crate::ai::client::ChatMessage {
                role: "system".to_string(),
                content: self.system_prompt.clone(),
            },
            crate::ai::client::ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "言語: {}\n\n要件:\n{}",
                    language, description
                ),
            },
        ];

        let code = self.ai_client.chat(&messages).await?;

        // Save the generated code
        self.store
            .update_coding_task_result(
                "temp",
                Some(&code),
                "completed",
                Some("コード生成完了"),
            )
            .await;

        info!("[Coding] Code generated successfully ({} chars)", code.len());
        Ok(code)
    }

    pub async fn generate_rust_code(&self, description: &str) -> Result<String, Box<dyn std::error::Error>> {
        self.generate_code(description, "Rust").await
    }

    pub async fn generate_python_code(&self, description: &str) -> Result<String, Box<dyn std::error::Error>> {
        self.generate_code(description, "Python").await
    }

    pub async fn generate_shell_script(&self, description: &str) -> Result<String, Box<dyn std::error::Error>> {
        self.generate_code(description, "Shell Script").await
    }
}
