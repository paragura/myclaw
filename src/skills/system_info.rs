#[derive(Clone)]
pub struct SystemInfoSkill;

impl SystemInfoSkill {
    pub fn new() -> Self {
        Self
    }

    pub async fn get_info(&self, args: &str) -> String {
        let info_type = args.trim();

        match info_type {
            "disk" => self.get_disk_info(),
            "memory" => self.get_memory_info(),
            "os" => self.get_os_info(),
            _ => self.get_all_info(),
        }
    }

    fn get_all_info(&self) -> String {
        let os = self.get_os_info();
        let disk = self.get_disk_info();
        let memory = self.get_memory_info();

        format!("**システム情報**\n\n{}\n\n{}\n\n{}", os, disk, memory)
    }

    fn get_os_info(&self) -> String {
        let os_type = std::env::consts::OS;
        let arch = std::env::consts::ARCH;

        let hostname = std::fs::read_to_string("/etc/hostname").ok();
        let hostname = hostname.as_deref().unwrap_or("unknown");

        format!(
            "**OS情報**\n\
            - タイプ: {}\n\
            - アーキテクチャ: {}\n\
            - ホスト名: {}",
            os_type, arch, hostname
        )
    }

    fn get_disk_info(&self) -> String {
        #[cfg(target_os = "macos")]
        {
            match std::process::Command::new("df").args(&["-h", "/"]).output() {
                Ok(output) => {
                    let lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .skip(1)
                        .map(|l| format!("  {}", l))
                        .collect();
                    format!(
                        "**ディスク情報 (/)**\n{}\n  ({}行)",
                        lines.join("\n"),
                        lines.len()
                    )
                }
                Err(e) => format!("ディスク情報取得エラー: {}", e),
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            "macOS以外のためディスク情報を取得できません。".to_string()
        }
    }

    fn get_memory_info(&self) -> String {
        #[cfg(target_os = "macos")]
        {
            match std::process::Command::new("vm_stat").output() {
                Ok(output) => {
                    let content = String::from_utf8_lossy(&output.stdout);
                    let lines: Vec<String> = content.lines().take(10).map(|l| format!("  {}", l)).collect();
                    format!(
                        "**メモリ情報 (vm_stat)**\n{}\n  ({}行)",
                        lines.join("\n"),
                        lines.len()
                    )
                }
                Err(e) => format!("メモリ情報取得エラー: {}", e),
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            "macOS以外のためメモリ情報を取得できません。".to_string()
        }
    }

    pub async fn list_processes(&self, filter: &str) -> String {
        match std::process::Command::new("ps")
            .args(&["-ax", "-o", "pid,user,%cpu,%mem,vsz,rss,tty,stat,start,time,command"])
            .output()
        {
            Ok(output) => {
                let content = String::from_utf8_lossy(&output.stdout);
                let lines: Vec<&str> = content.lines().collect();

                let header = lines.first().copied().unwrap_or("");
                let filtered_lines: Vec<String> = if filter.is_empty() {
                    lines.iter().skip(1).take(20).map(|l| format!("  {}", l)).collect()
                } else {
                    lines.iter()
                        .skip(1)
                        .filter(|l| l.to_lowercase().contains(&filter.to_lowercase()))
                        .take(20)
                        .map(|l| format!("  {}", l))
                        .collect()
                };

                format!(
                    "**プロセス一覧** ({}件)\n\n\
                    {}\n\n\
                    {}",
                    filtered_lines.len(),
                    header,
                    filtered_lines.join("\n")
                )
            }
            Err(e) => format!("プロセス一覧取得エラー: {}", e),
        }
    }
}
