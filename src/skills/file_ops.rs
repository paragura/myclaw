use std::fs;
use tracing::{debug, error};
use std::sync::Arc;

use crate::memory::store::MemoryStore;

#[derive(Clone)]
pub struct FileOpSkill;

impl FileOpSkill {
    pub fn new() -> Self {
        Self
    }

    pub async fn read_file(&self, args: &str, _store: &Arc<MemoryStore>) -> String {
        let filepath = args.trim();
        if filepath.is_empty() {
            return "ファイルパスを入力してください: `!skill file_read <filepath>`".to_string();
        }

        match fs::read_to_string(filepath) {
            Ok(content) => {
                debug!("[FileOp] Read file: {}", filepath);
                let lines = content.lines().count();
                if content.len() > 3000 {
                    format!(
                        "📄 **{}** ({}行, {}バイト)\n```\n{}\n...\n```",
                        filepath,
                        lines,
                        content.len(),
                        content.chars().take(2000).collect::<String>()
                    )
                } else {
                    format!("📄 **{}** ({}行)\n```\n{}\n```", filepath, lines, content)
                }
            }
            Err(e) => {
                error!("[FileOp] Read error: {}", e);
                format!("ファイルの読み込みに失敗しました: `{}`\nエラー: {}", filepath, e)
            }
        }
    }

    pub async fn list_dir(&self, args: &str, _store: &Arc<MemoryStore>) -> String {
        let dirpath = args.trim();
        if dirpath.is_empty() {
            return "ディレクトリパスを入力してください: `!skill file_list <directory>`".to_string();
        }

        let entries = match fs::read_dir(dirpath) {
            Ok(rd) => rd.filter_map(|e| e.ok()).collect::<Vec<_>>(),
            Err(e) => {
                error!("[FileOp] List error: {}", e);
                return format!("ディレクトリの読み込みに失敗しました: `{}`\nエラー: {}", dirpath, e);
            }
        };

        if entries.is_empty() {
            return format!("📂 **{}** は空です。", dirpath);
        }

        let mut files = Vec::new();
        let mut dirs = Vec::new();
        let mut total_size = 0u64;

        for entry in &entries {
            let name = entry.file_name().to_string_lossy().to_string();
            let metadata = entry.metadata();
            match metadata {
                Ok(meta) => {
                    if meta.is_dir() {
                        dirs.push(name);
                    } else {
                        files.push(format!("  {} ({:?})", name, meta.len()));
                        total_size += meta.len();
                    }
                }
                Err(_) => files.push(format!("  {} (unknown)", name)),
            }
        }

        format!(
            "📂 **{}** ({}個のエントリ: {}個のファイル, {}個のディレクトリ, 合計 {:?})\n\n\
            ディレクトリ:\n{}\n\n\
            ファイル:\n{}",
            dirpath,
            entries.len(),
            files.len(),
            dirs.len(),
            total_size,
            dirs.join("\n"),
            files.join("\n")
        )
    }
}
