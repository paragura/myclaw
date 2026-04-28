/// Determine whether a shell command is known to be safe (read-only).
///
/// Based on Codex's two-tier safety model:
/// - Safe commands run without user approval
/// - Dangerous commands require explicit confirmation
///
/// This module provides `is_known_safe()` and `is_dangerous()` predicates
/// that operate on the command string before it is executed.


/// Commands that are always safe (read-only, no side effects).
const SAFE_COMMANDS: &[&str] = &[
    // Navigation / listing
    "ls", "dir", "cd", "pwd", "find", "tree", "stat", "file", "which", "where",
    // File content
    "cat", "head", "tail", "less", "more", "wc", "grep", "egrep", "fgrep", "ripgrep", "rg",
    // Diff / comparison
    "diff", "cmp", "comm", "sort", "uniq", "cut", "tr", "sed",
    // Git (read-only subset)
    "git",
    // Misc read-only
    "echo", "printf", "date", "time", "uptime", "whoami", "id", "hostname",
    "uname", "env", "printenv", "printenv", "export",
    "ps", "top", "htop", "df", "du", "free",
    "hostname", "ip", "ping", "curl", "wget",
    "date", "cal", "bc", "seq", "yes", "true", "false",
    // Editors (read-only invocation)
    "jq", "yq", "python3", "python", "node", "npm", "cargo",
];

/// Arguments that make otherwise-safe commands dangerous.
const DANGEROUS_FLAGS: &[&str] = &[
    "-i",  // in-place edit (sed, grep)
    "-e",  // execute (find)
    "-exec", // execute (find)
    "-delete", // delete (find)
    "-remove",  // remove
    "-modify",   // modify
];

/// Commands that are inherently dangerous (destructive or state-changing).
const DANGEROUS_COMMANDS: &[&str] = &[
    "rm", "rmdir", "mv", "cp", "touch", "mkdir", "chmod", "chown", "chgrp",
    "kill", "pkill", "killall",
    "apt", "apt-get", "yum", "dnf", "brew", "pip", "pip3",
    "cargo", "npm", "yarn", "pnpm",
    "make", "cmake", "configure",
    "dd", "truncate", "truncate", "shred",
    "sudo", "su", "passwd",
];

/// Arguments that make git commands dangerous.
const DANGEROUS_GIT_ARGS: &[&str] = &[
    "checkout", "commit", "push", "merge", "rebase", "reset", "amend",
    "force", "delete", "remove", "clone", "init", "tag",
];

/// Classify a shell command as safe, potentially dangerous, or unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyLevel {
    /// The command is known to be read-only and non-destructive.
    Safe,
    /// The command may have side effects or be destructive.
    Dangerous,
    /// The command cannot be classified (not in either list).
    Unknown,
}

impl std::fmt::Display for SafetyLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SafetyLevel::Safe => write!(f, "安全"),
            SafetyLevel::Dangerous => write!(f, "危険"),
            SafetyLevel::Unknown => write!(f, "不明"),
        }
    }
}

/// Classify the given command string into a safety level.
pub fn assess_safety(cmd: &str) -> SafetyLevel {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return SafetyLevel::Unknown;
    }

    let program = parts[0];
    let args = &parts[1..];

    // Check dangerous commands first (some overlap with safe)
    if is_dangerous_program(program) {
        return SafetyLevel::Dangerous;
    }

    // Check safe commands
    if SAFE_COMMANDS.contains(&program) {
        // Check for dangerous arguments
        if has_dangerous_args(program, args) {
            return SafetyLevel::Dangerous;
        }
        return SafetyLevel::Safe;
    }

    SafetyLevel::Unknown
}

fn is_dangerous_program(program: &str) -> bool {
    DANGEROUS_COMMANDS.iter().any(|&d| d == program)
}

fn has_dangerous_args(program: &str, args: &[&str]) -> bool {
    // Git special handling
    if program == "git" {
        for arg in args {
            if DANGEROUS_GIT_ARGS.iter().any(|&d| d == *arg) {
                return true;
            }
        }
        return false;
    }

    // General dangerous flags
    args.iter().any(|arg| {
        for flag in DANGEROUS_FLAGS {
            if *arg == *flag || arg.starts_with(&format!("-{}", flag)) {
                return true;
            }
        }
        // Check for `-i=`, `-e=`, etc.
        if arg.starts_with('-') && *arg != "-" {
            // Flags like `-i backup.txt` or `--in-place`
            let flag = arg.trim_start_matches('-');
            for dangerous_flag in DANGEROUS_FLAGS {
                if flag.starts_with(dangerous_flag) {
                    return true;
                }
            }
        }
        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_ls() {
        assert_eq!(assess_safety("ls -la"), SafetyLevel::Safe);
    }

    #[test]
    fn test_safe_grep() {
        assert_eq!(assess_safety("grep -r error"), SafetyLevel::Safe);
    }

    #[test]
    fn test_safe_cat() {
        assert_eq!(assess_safety("cat file.txt"), SafetyLevel::Safe);
    }

    #[test]
    fn test_safe_git_status() {
        assert_eq!(assess_safety("git status"), SafetyLevel::Safe);
    }

    #[test]
    fn test_safe_curl() {
        assert_eq!(assess_safety("curl -s https://example.com"), SafetyLevel::Safe);
    }

    #[test]
    fn test_safe_echo() {
        assert_eq!(assess_safety("echo hello"), SafetyLevel::Safe);
    }

    #[test]
    fn test_dangerous_rm() {
        assert_eq!(assess_safety("rm -rf /tmp/old"), SafetyLevel::Dangerous);
    }

    #[test]
    fn test_dangerous_mv() {
        assert_eq!(assess_safety("mv a.txt b.txt"), SafetyLevel::Dangerous);
    }

    #[test]
    fn test_dangerous_sed_inplace() {
        assert_eq!(assess_safety("sed -i 's/old/new/' file"), SafetyLevel::Dangerous);
    }

    #[test]
    fn test_dangerous_git_checkout() {
        assert_eq!(assess_safety("git checkout main"), SafetyLevel::Dangerous);
    }

    #[test]
    fn test_dangerous_git_push() {
        assert_eq!(assess_safety("git push"), SafetyLevel::Dangerous);
    }

    #[test]
    fn test_dangerous_git_commit() {
        assert_eq!(assess_safety("git commit -m 'fix'"), SafetyLevel::Dangerous);
    }

    #[test]
    fn test_dangerous_sudo() {
        assert_eq!(assess_safety("sudo apt update"), SafetyLevel::Dangerous);
    }

    #[test]
    fn test_unknown_command() {
        assert_eq!(assess_safety("my_custom_script --flag"), SafetyLevel::Unknown);
    }

    #[test]
    fn test_empty_command() {
        assert_eq!(assess_safety(""), SafetyLevel::Unknown);
    }

    #[test]
    fn test_git_log_is_safe() {
        assert_eq!(assess_safety("git log --oneline -5"), SafetyLevel::Safe);
    }

    #[test]
    fn test_find_without_delete_is_safe() {
        assert_eq!(assess_safety("find . -name '*.txt'"), SafetyLevel::Safe);
    }

    #[test]
    fn test_safety_display() {
        assert_eq!(format!("{}", SafetyLevel::Safe), "安全");
        assert_eq!(format!("{}", SafetyLevel::Dangerous), "危険");
        assert_eq!(format!("{}", SafetyLevel::Unknown), "不明");
    }
}
