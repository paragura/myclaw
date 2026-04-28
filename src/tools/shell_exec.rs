/// Execute a shell command and return stdout/stderr.
use std::process::Command;

use super::command_safety::{self, SafetyLevel};

#[derive(Clone)]
pub struct ShellExecTool {
    /// If true, dangerous commands will report a warning rather than executing.
    /// In Discord mode, this enables a confirm-before-run flow.
    pub confirm_dangerous: bool,
}

impl ShellExecTool {
    pub fn new() -> Self {
        Self {
            confirm_dangerous: false,
        }
    }

    pub fn execute(&self, args: &str) -> String {
        let cmd = args.trim();
        if cmd.is_empty() {
            return "コマンドを入力してください: `!skill shell_exec ls -la`".to_string();
        }

        // Safety check
        let safety = command_safety::assess_safety(cmd);

        // Split into command and arguments
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            return "コマンドを入力してください: `!skill shell_exec ls -la`".to_string();
        }

        // If the command is dangerous and confirmation is required, warn
        if self.confirm_dangerous && safety == SafetyLevel::Dangerous {
            return format!(
                "⚠️ 危険なコマンドです: `{}\n安全性: {}\n確認が必要です。",
                cmd, safety
            );
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
                    "**コマンド:** `{} `{}`\n**安全性:** {}\n**終了コード:** {}\n",
                    if exit_code == 0 { "✅" } else { "❌" },
                    cmd,
                    safety,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_exec_empty_args() {
        let tool = ShellExecTool::new();
        let result = tool.execute("");
        assert!(result.contains("コマンドを入力"));
    }

    #[test]
    fn test_shell_exec_whitespace_only() {
        let tool = ShellExecTool::new();
        let result = tool.execute("   ");
        assert!(result.contains("コマンドを入力"));
    }

    #[test]
    fn test_shell_exec_echo() {
        let tool = ShellExecTool::new();
        let result = tool.execute("echo hello");
        assert!(result.contains("✅"));
        assert!(result.contains("hello"));
    }

    #[test]
    fn test_shell_exec_command_not_found() {
        let tool = ShellExecTool::new();
        let result = tool.execute("nonexistent_command_xyz123");
        assert!(result.contains("コマンドが見つかりません"));
    }

    #[test]
    fn test_shell_exec_exit_code() {
        let tool = ShellExecTool::new();
        let result = tool.execute("false");
        assert!(result.contains("❌"));
    }

    #[test]
    fn test_shell_exec_true_exit_code() {
        let tool = ShellExecTool::new();
        let result = tool.execute("true");
        assert!(result.contains("✅"));
    }

    #[test]
    fn test_truncate_shorter() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_longer() {
        let result = truncate("hello world", 5);
        assert_eq!(result, "hello...");
    }

    #[test]
    fn test_truncate_zero() {
        let result = truncate("hello", 0);
        assert_eq!(result, "...");
    }

    #[test]
    fn test_truncate_multibyte() {
        let result = truncate("こんにちは世界", 5);
        assert_eq!(result, "こんにちは...");
    }
}
