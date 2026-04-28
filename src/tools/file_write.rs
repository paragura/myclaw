/// Write content to a file.
use std::fs;
use std::io::Write;

#[derive(Clone)]
pub struct FileWriteTool;

impl FileWriteTool {
    pub fn new() -> Self {
        Self
    }

    pub fn write(&self, args: &str) -> String {
        // Parse: "filepath content"
        let args = args.trim();
        if args.is_empty() {
            return "引数を入力してください: `!skill file_write <filepath> <content>`".to_string();
        }

        // Split on first whitespace to get filepath
        let parts: Vec<&str> = args.splitn(2, ' ').collect();
        if parts.len() < 2 {
            return "ファイルパスと内容を入力してください: `!skill file_write /tmp/test.txt hello`".to_string();
        }

        let filepath = parts[0];
        let content = parts[1];
        let exists = std::fs::metadata(filepath).is_ok();

        // Create parent directory if needed
        if let Some(parent) = std::path::Path::new(filepath).parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(e) = fs::create_dir_all(parent) {
                    return format!("ディレクトリの作成に失敗しました: `{}`\nエラー: {}", parent.display(), e);
                }
            }
        }

        let mut file = match fs::File::create(filepath) {
            Ok(f) => f,
            Err(e) => return format!("ファイルの作成に失敗しました: `{}`\nエラー: {}", filepath, e),
        };

        if let Err(e) = file.write_all(content.as_bytes()) {
            return format!("ファイルの書き込みに失敗しました: `{}`\nエラー: {}", filepath, e);
        }

        let status = if exists { "更新" } else { "作成" };
        format!(
            "✅ ファイル{}: `{}` ({}文字)",
            status,
            filepath,
            content.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_file_write_empty_args() {
        let tool = FileWriteTool::new();
        let result = tool.write("");
        assert!(result.contains("引数を入力"));
    }

    #[test]
    fn test_file_write_missing_content() {
        let tool = FileWriteTool::new();
        let result = tool.write("just_a_path.txt");
        assert!(result.contains("ファイルパスと内容"));
    }

    #[test]
    fn test_file_write_new_file() {
        let dir = TempDir::new().unwrap();
        let filepath = dir.path().join("new_file.txt");

        let tool = FileWriteTool::new();
        let result = tool.write(&format!("{} hello world", filepath.display()));
        assert!(result.contains("作成"));
        assert!(result.contains("✅"));

        // Verify file actually exists
        assert!(fs::read_to_string(&filepath).unwrap() == "hello world");
    }

    #[test]
    fn test_file_write_update_existing() {
        let dir = TempDir::new().unwrap();
        let filepath = dir.path().join("existing.txt");
        fs::write(&filepath, "old content").unwrap();

        let tool = FileWriteTool::new();
        let result = tool.write(&format!("{} new content", filepath.display()));
        assert!(result.contains("更新"));

        let content = fs::read_to_string(&filepath).unwrap();
        assert_eq!(content, "new content");
    }

    #[test]
    fn test_file_write_creates_parent_dir() {
        let dir = TempDir::new().unwrap();
        let filepath = dir.path().join("nested").join("sub").join("file.txt");

        let tool = FileWriteTool::new();
        let result = tool.write(&format!("{} content", filepath.display()));
        assert!(result.contains("✅"));

        assert!(fs::read_to_string(&filepath).unwrap() == "content");
    }
}
