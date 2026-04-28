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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_file_read_empty_args() {
        let tool = FileReadTool::new();
        let result = tool.read("");
        assert!(result.contains("ファイルパス"));
    }

    #[test]
    fn test_file_read_nonexistent_file() {
        let tool = FileReadTool::new();
        let result = tool.read("/nonexistent/path/to/file.txt");
        assert!(result.contains("読み込みに失敗"));
    }

    #[test]
    fn test_file_read_existing_file() {
        let dir = TempDir::new().unwrap();
        let filepath = dir.path().join("test.txt");
        fs::write(&filepath, "hello world").unwrap();

        let tool = FileReadTool::new();
        let result = tool.read(filepath.to_str().unwrap());
        assert!(result.contains("hello world"));
        assert!(result.contains("✅"));
    }

    #[test]
    fn test_file_read_truncation() {
        let dir = TempDir::new().unwrap();
        let filepath = dir.path().join("large.txt");
        let content = "a".repeat(4000);
        fs::write(&filepath, &content).unwrap();

        let tool = FileReadTool::new();
        let result = tool.read(filepath.to_str().unwrap());
        assert!(result.contains("..."));
    }
}
