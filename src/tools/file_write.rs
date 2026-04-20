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
