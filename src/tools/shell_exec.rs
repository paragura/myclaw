/// Execute a shell command and return stdout/stderr.
use std::process::Command;

#[derive(Clone)]
pub struct ShellExecTool;

impl ShellExecTool {
    pub fn new() -> Self {
        Self
    }

    pub fn execute(&self, args: &str) -> String {
        let cmd = args.trim();
        if cmd.is_empty() {
            return "コマンドを入力してください: `!skill shell_exec ls -la`".to_string();
        }

        // Split into command and arguments
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            return "コマンドを入力してください: `!skill shell_exec ls -la`".to_string();
        }

        let output = Command::new(parts[0])
            .args(&parts[1..])
            .output();

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let exit_code = output.status.code().unwrap_or(-1);

                let mut result = format!(
                    "**コマンド:** `{} `{}`\n**終了コード:** {}\n",
                    if exit_code == 0 { "✅" } else { "❌" },
                    cmd,
                    exit_code
                );

                if !stdout.is_empty() {
                    let truncated = truncate(&stdout, 3000);
                    result.push_str(&format!("**stdout:**\n```\n{}\n```", truncated));
                }
                if !stderr.is_empty() {
                    let truncated = truncate(&stderr, 1500);
                    result.push_str(&format!("\n**stderr:**\n```\n{}\n```", truncated));
                }
                if stdout.is_empty() && stderr.is_empty() {
                    result.push_str("(出力なし)");
                }
                result
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    format!("コマンドが見つかりません: `{}`\nエラー: {}", parts[0], e)
                } else {
                    format!("コマンド実行エラー: `{}`\nエラー: {}", cmd, e)
                }
            }
        }
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.len() <= max {
        text.to_string()
    } else {
        format!("{}...", text.chars().take(max).collect::<String>())
    }
}
