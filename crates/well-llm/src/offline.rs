//! offline.rs: High-Performance Zero-Network Semantic Translation Engine
//!
//! Provides instant, offline natural-language-to-shell translation for common
//! Git, Docker, Cargo, process management, file, and networking commands.

use regex::Regex;

pub struct OfflineTranslation {
    pub command: String,
    pub explanation: String,
    pub confidence: f32,
}

pub struct OfflineRuleEngine;

impl OfflineRuleEngine {
    /// Translates a natural language query into an idiomatic shell command.
    pub fn translate(query: &str) -> Option<OfflineTranslation> {
        let q = query.trim().to_lowercase();

        // 1. Process & Port Management
        if let Some(caps) =
            Regex::new(r"(?:kill|terminate|stop)\s+(?:process\s+)?(?:on\s+)?port\s+(\d+)")
                .ok()?
                .captures(&q)
        {
            let port = &caps[1];
            return Some(OfflineTranslation {
                command: format!("lsof -ti:{port} | xargs kill -9"),
                explanation: format!("Finds PID listening on TCP port {port} and forces SIGKILL."),
                confidence: 0.98,
            });
        }

        if let Some(caps) = Regex::new(r"(?:find|check|show|what is)\s+(?:using|on)\s+port\s+(\d+)")
            .ok()?
            .captures(&q)
        {
            let port = &caps[1];
            return Some(OfflineTranslation {
                command: format!("lsof -i :{port}"),
                explanation: format!("Lists process names and PIDs bound to port {port}."),
                confidence: 0.98,
            });
        }

        if q.contains("top processes by memory")
            || q.contains("processes using most memory")
            || q.contains("most ram")
        {
            return Some(OfflineTranslation {
                command: "ps aux --sort=-%mem | head -n 10".to_string(),
                explanation: "Lists top 10 processes consuming the highest resident memory."
                    .to_string(),
                confidence: 0.95,
            });
        }

        if q.contains("top processes by cpu")
            || q.contains("processes using most cpu")
            || q.contains("highest cpu")
        {
            return Some(OfflineTranslation {
                command: "ps aux --sort=-%cpu | head -n 10".to_string(),
                explanation: "Lists top 10 processes consuming the highest CPU time.".to_string(),
                confidence: 0.95,
            });
        }

        // 2. Git Workflows
        if q.contains("revert last commit keep changes")
            || q.contains("undo commit keep staged")
            || q.contains("undo last commit keep changes")
        {
            return Some(OfflineTranslation {
                command: "git reset --soft HEAD~1".to_string(),
                explanation: "Undoes the latest commit while preserving all modified files in the staging area.".to_string(),
                confidence: 0.99,
            });
        }

        if q.contains("undo last commit completely")
            || q.contains("discard last commit")
            || q.contains("nuke last commit")
        {
            return Some(OfflineTranslation {
                command: "git reset --hard HEAD~1".to_string(),
                explanation: "Permanently discards the latest commit and all working tree changes."
                    .to_string(),
                confidence: 0.99,
            });
        }

        if q.contains("discard all changes")
            || q.contains("discard unstaged")
            || q.contains("revert all changes")
        {
            return Some(OfflineTranslation {
                command: "git restore .".to_string(),
                explanation:
                    "Discards all uncommitted changes in tracked files within the working tree."
                        .to_string(),
                confidence: 0.97,
            });
        }

        if q.contains("clean untracked")
            || q.contains("delete untracked files")
            || q.contains("remove untracked")
        {
            return Some(OfflineTranslation {
                command: "git clean -fd".to_string(),
                explanation:
                    "Recursively deletes untracked files and directories from the repository."
                        .to_string(),
                confidence: 0.96,
            });
        }

        if let Some(caps) =
            Regex::new(r"(?:create|checkout|switch to)\s+(?:new\s+)?branch\s+([a-zA-Z0-9_\-\./]+)")
                .ok()?
                .captures(&q)
        {
            let branch = &caps[1];
            return Some(OfflineTranslation {
                command: format!("git checkout -b {branch}"),
                explanation: format!("Creates and switches to a new Git branch '{branch}'."),
                confidence: 0.98,
            });
        }

        if q.contains("commit history graph")
            || q.contains("git graph")
            || q.contains("visualize commits")
        {
            return Some(OfflineTranslation {
                command: "git log --oneline --graph --decorate --all".to_string(),
                explanation: "Displays visual ASCII topology graph of commits across all branches."
                    .to_string(),
                confidence: 0.96,
            });
        }

        if q.contains("amend last commit") || q.contains("add to last commit") {
            return Some(OfflineTranslation {
                command: "git commit --amend --no-edit".to_string(),
                explanation: "Combines currently staged changes into the previous commit without editing commit message.".to_string(),
                confidence: 0.96,
            });
        }

        // 3. File & Search Operations
        if let Some(caps) =
            Regex::new(r"(?:find|search)\s+(?:files\s+)?larger\s+than\s+(\d+)\s*(mb|gb|kb|m|g|k)")
                .ok()?
                .captures(&q)
        {
            let num = &caps[1];
            let unit = caps[2].chars().next().unwrap_or('m').to_ascii_uppercase();
            return Some(OfflineTranslation {
                command: format!("find . -type f -size +{num}{unit}"),
                explanation: format!("Recursively finds files larger than {num}{unit} in the current directory tree."),
                confidence: 0.97,
            });
        }

        if q.contains("modified in last 24 hours")
            || q.contains("changed today")
            || q.contains("modified today")
        {
            return Some(OfflineTranslation {
                command: "find . -type f -mtime -1".to_string(),
                explanation: "Finds files modified within the past 24 hours.".to_string(),
                confidence: 0.95,
            });
        }

        if let Some(caps) = Regex::new(r#"(?:search for|find)\s+(?:text\s+)?["']([^"']+)["']"#)
            .ok()?
            .captures(&q)
        {
            let text = &caps[1];
            return Some(OfflineTranslation {
                command: format!("grep -rn \"{text}\" ."),
                explanation: format!(
                    "Recursively searches for '{text}' across all files with line numbers."
                ),
                confidence: 0.94,
            });
        }

        if q.contains("count lines of code") || q.contains("line count") {
            return Some(OfflineTranslation {
                command: "find . -type f \\( -name '*.rs' -o -name '*.dart' -o -name '*.js' -o -name '*.ts' \\) -exec wc -l {} +".to_string(),
                explanation: "Counts total lines across code files in current workspace.".to_string(),
                confidence: 0.93,
            });
        }

        if q.contains("disk space") || q.contains("free space") || q.contains("disk usage") {
            return Some(OfflineTranslation {
                command: "df -h".to_string(),
                explanation: "Displays mounted filesystem capacity and available space in human-readable units.".to_string(),
                confidence: 0.98,
            });
        }

        if q.contains("folder size")
            || q.contains("directory size")
            || q.contains("how big is this folder")
        {
            return Some(OfflineTranslation {
                command: "du -sh .".to_string(),
                explanation: "Calculates total disk usage of current folder.".to_string(),
                confidence: 0.97,
            });
        }

        // 4. Archive & Compression
        if let Some(caps) = Regex::new(r"(?:compress|tar|zip)\s+(?:folder\s+|dir\s+)?([a-zA-Z0-9_\-\.]+)\s+to\s+([a-zA-Z0-9_\-\.]+)").ok()?.captures(&q) {
            let src = &caps[1];
            let dst = &caps[2];
            return Some(OfflineTranslation {
                command: format!("tar -czvf {dst} {src}"),
                explanation: format!("Creates gzip-compressed archive '{dst}' from '{src}'."),
                confidence: 0.96,
            });
        }

        if let Some(caps) =
            Regex::new(r"(?:extract|unzip|untar)\s+([a-zA-Z0-9_\-\.]+\.(?:tar\.gz|tgz|tar))")
                .ok()?
                .captures(&q)
        {
            let archive = &caps[1];
            return Some(OfflineTranslation {
                command: format!("tar -xzvf {archive}"),
                explanation: format!(
                    "Extracts contents of tarball '{archive}' into current working directory."
                ),
                confidence: 0.97,
            });
        }

        // 5. Docker
        if q.contains("list running containers") || q.contains("running docker") {
            return Some(OfflineTranslation {
                command: "docker ps".to_string(),
                explanation: "Displays all currently active Docker containers.".to_string(),
                confidence: 0.98,
            });
        }

        if q.contains("list all containers") || q.contains("all docker containers") {
            return Some(OfflineTranslation {
                command: "docker ps -a".to_string(),
                explanation:
                    "Displays all Docker containers including stopped or exited instances."
                        .to_string(),
                confidence: 0.98,
            });
        }

        if q.contains("stop all containers") || q.contains("kill all docker containers") {
            return Some(OfflineTranslation {
                command: "docker stop $(docker ps -q)".to_string(),
                explanation: "Sends SIGTERM to all running Docker container IDs.".to_string(),
                confidence: 0.95,
            });
        }

        if q.contains("docker clean")
            || q.contains("docker prune")
            || q.contains("clean docker images")
        {
            return Some(OfflineTranslation {
                command: "docker system prune -a --volumes".to_string(),
                explanation:
                    "Removes unused containers, dangling networks, and unreferenced build cache."
                        .to_string(),
                confidence: 0.95,
            });
        }

        // 6. Rust & Cargo
        if q.contains("check compilation") || q.contains("cargo check") {
            return Some(OfflineTranslation {
                command: "cargo check --workspace".to_string(),
                explanation:
                    "Rapidly validates code syntax and type checks without running code generator."
                        .to_string(),
                confidence: 0.99,
            });
        }

        if q.contains("run tests") || q.contains("cargo test") {
            return Some(OfflineTranslation {
                command: "cargo test --workspace -- --nocapture".to_string(),
                explanation: "Runs all unit/integration tests with live stdout streaming."
                    .to_string(),
                confidence: 0.98,
            });
        }

        // 7. Network
        if q.contains("my ip") || q.contains("public ip") || q.contains("what is my ip") {
            return Some(OfflineTranslation {
                command: "curl -s ifconfig.me".to_string(),
                explanation: "Queries external IP echo service over HTTPS to return public WAN IP."
                    .to_string(),
                confidence: 0.97,
            });
        }

        if let Some(caps) = Regex::new(r"(?:ping|test connection to)\s+([a-zA-Z0-9_\-\.]+)")
            .ok()?
            .captures(&q)
        {
            let host = &caps[1];
            return Some(OfflineTranslation {
                command: format!("ping -c 4 {host}"),
                explanation: format!(
                    "Sends 4 ICMP ECHO packets to test network reachability to {host}."
                ),
                confidence: 0.98,
            });
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_killing_rule() {
        let res = OfflineRuleEngine::translate("kill process on port 8080").unwrap();
        assert_eq!(res.command, "lsof -ti:8080 | xargs kill -9");
        assert!(res.confidence >= 0.95);
    }

    #[test]
    fn test_git_soft_reset() {
        let res = OfflineRuleEngine::translate("revert last commit keep changes").unwrap();
        assert_eq!(res.command, "git reset --soft HEAD~1");
    }

    #[test]
    fn test_file_size_search() {
        let res = OfflineRuleEngine::translate("find files larger than 100mb").unwrap();
        assert_eq!(res.command, "find . -type f -size +100M");
    }

    #[test]
    fn test_docker_commands() {
        let res = OfflineRuleEngine::translate("list running docker containers").unwrap();
        assert_eq!(res.command, "docker ps");
    }
}
