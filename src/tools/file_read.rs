/// Read file contents.
use std::fs;

#[derive(Clone)]
pub struct FileReadTool;

impl FileReadTool {
    pub fn new() -> Self {
        Self
    }

    pub fn read(&self, args: &str) -> String {
        let filepath = args.trim();
        if filepath.is_empty() {
            return "ファイルパスを入力してください: `!skill file_read /path/to/file`".to_string();
        }

        let content = match fs::read_to_string(filepath) {
            Ok(c) => c,
            Err(e) => return format!("ファイルの読み込みに失敗しました: `{}`\nエラー: {}", filepath, e),
        };

        let len = content.len();
        let truncated = if len > 3000 {
            format!("{}...", content.chars().take(3000).collect::<String>())
        } else {
            content.clone()
        };

        format!(
            "✅ ファイル読み込み: `{}` ({}文字)\n```\n{}\n```",
            filepath,
            len,
            truncated
        )
    }
}
