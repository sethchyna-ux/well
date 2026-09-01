//! HyperShell: High-Throughput Asynchronous Pipeline & Task Engine for Well
//!
//! Subsystems:
//! - HyperShellEngine: Async multi-stage command pipeline runner and background task queue.
//! - PipelineStage: Composable command pipeline node with stdin/stdout streaming.
//! - TaskResult: Structured telemetry output including microsecond latency profiling.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::sync::Mutex;
use crate::MetisHistory;

/// Microsecond-precision result of a HyperShell execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: u64,
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub wall_time_us: u128,
    pub success: bool,
}

/// A single stage in an asynchronous pipeline
#[derive(Debug, Clone)]
pub struct PipelineStage {
    pub program: String,
    pub args: Vec<String>,
}

impl PipelineStage {
    pub fn new(cmd: &str) -> Self {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let program = parts.first().unwrap_or(&"").to_string();
        let args = if parts.len() > 1 {
            parts[1..].iter().map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        };

        Self { program, args }
    }
}

/// HyperShell asynchronous engine
pub struct HyperShellEngine {
    next_task_id: AtomicU64,
    current_dir: PathBuf,
    history: Arc<Mutex<MetisHistory>>,
    active_jobs: Arc<Mutex<HashMap<u64, TaskResult>>>,
}

impl Default for HyperShellEngine {
    fn default() -> Self {
        Self::new(PathBuf::from("."))
    }
}

impl HyperShellEngine {
    pub fn new(current_dir: PathBuf) -> Self {
        Self {
            next_task_id: AtomicU64::new(1),
            current_dir,
            history: Arc::new(Mutex::new(MetisHistory::new())),
            active_jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn with_history(current_dir: PathBuf, history: Arc<Mutex<MetisHistory>>) -> Self {
        Self {
            next_task_id: AtomicU64::new(1),
            current_dir,
            history,
            active_jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Execute a single command asynchronously with microsecond telemetry
    pub async fn execute_cmd(&self, raw_cmd: &str) -> TaskResult {
        let task_id = self.next_task_id.fetch_add(1, Ordering::SeqCst);
        let trimmed = raw_cmd.trim();

        {
            let mut hist = self.history.lock().await;
            hist.learn_command(trimmed);
        }

        let start = Instant::now();

        let output = Command::new("/bin/sh")
            .arg("-c")
            .arg(trimmed)
            .current_dir(&self.current_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        let wall_time_us = start.elapsed().as_micros();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let exit_code = out.status.code().unwrap_or(-1);
                let success = out.status.success();

                TaskResult {
                    task_id,
                    command: trimmed.to_string(),
                    stdout,
                    stderr,
                    exit_code,
                    wall_time_us,
                    success,
                }
            }
            Err(e) => TaskResult {
                task_id,
                command: trimmed.to_string(),
                stdout: String::new(),
                stderr: format!("Execution failure: {}", e),
                exit_code: -1,
                wall_time_us,
                success: false,
            },
        }
    }

    /// Execute a multi-stage piped pipeline asynchronously (e.g. `cat file | grep key | wc -l`)
    pub async fn execute_pipeline(&self, stages: &[&str]) -> TaskResult {
        let task_id = self.next_task_id.fetch_add(1, Ordering::SeqCst);
        let full_pipeline_str = stages.join(" | ");

        {
            let mut hist = self.history.lock().await;
            hist.learn_command(&full_pipeline_str);
        }

        let start = Instant::now();

        if stages.is_empty() {
            return TaskResult {
                task_id,
                command: full_pipeline_str,
                stdout: String::new(),
                stderr: "Empty pipeline stages".to_string(),
                exit_code: 0,
                wall_time_us: 0,
                success: true,
            };
        }

        // Run through shell pipeline directly for robustness
        let output = Command::new("/bin/sh")
            .arg("-c")
            .arg(&full_pipeline_str)
            .current_dir(&self.current_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        let wall_time_us = start.elapsed().as_micros();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let exit_code = out.status.code().unwrap_or(-1);
                let success = out.status.success();

                TaskResult {
                    task_id,
                    command: full_pipeline_str,
                    stdout,
                    stderr,
                    exit_code,
                    wall_time_us,
                    success,
                }
            }
            Err(e) => TaskResult {
                task_id,
                command: full_pipeline_str,
                stdout: String::new(),
                stderr: format!("Pipeline error: {}", e),
                exit_code: -1,
                wall_time_us,
                success: false,
            },
        }
    }

    /// Spawn an asynchronous task into the background and track its completion in the task map
    pub async fn spawn_background_task(&self, cmd: &str) -> u64 {
        let task_id = self.next_task_id.fetch_add(1, Ordering::SeqCst);
        let cmd_owned = cmd.to_string();
        let cur_dir = self.current_dir.clone();
        let active_jobs = Arc::clone(&self.active_jobs);
        let history = Arc::clone(&self.history);

        tokio::spawn(async move {
            {
                let mut hist = history.lock().await;
                hist.learn_command(&cmd_owned);
            }

            let start = Instant::now();
            let output = Command::new("/bin/sh")
                .arg("-c")
                .arg(&cmd_owned)
                .current_dir(&cur_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await;

            let wall_time_us = start.elapsed().as_micros();

            let res = match output {
                Ok(out) => TaskResult {
                    task_id,
                    command: cmd_owned,
                    stdout: String::from_utf8_lossy(&out.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&out.stderr).to_string(),
                    exit_code: out.status.code().unwrap_or(-1),
                    wall_time_us,
                    success: out.status.success(),
                },
                Err(e) => TaskResult {
                    task_id,
                    command: cmd_owned,
                    stdout: String::new(),
                    stderr: format!("Background error: {}", e),
                    exit_code: -1,
                    wall_time_us,
                    success: false,
                },
            };

            let mut jobs = active_jobs.lock().await;
            jobs.insert(task_id, res);
        });

        task_id
    }

    /// Retrieve the completed result of a background job
    pub async fn get_job_result(&self, task_id: u64) -> Option<TaskResult> {
        let jobs = self.active_jobs.lock().await;
        jobs.get(&task_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hypershell_single_cmd() {
        let engine = HyperShellEngine::default();
        let res = engine.execute_cmd("echo 'Hello HyperShell'").await;

        assert!(res.success);
        assert_eq!(res.exit_code, 0);
        assert!(res.stdout.contains("Hello HyperShell"));
        assert!(res.wall_time_us > 0);
    }

    #[tokio::test]
    async fn test_hypershell_pipeline() {
        let engine = HyperShellEngine::default();
        let stages = ["echo -e 'alpha\\nbeta\\ngamma'", "grep 'beta'"];
        let res = engine.execute_pipeline(&stages).await;

        assert!(res.success);
        assert_eq!(res.exit_code, 0);
        assert_eq!(res.stdout.trim(), "beta");
    }

    #[tokio::test]
    async fn test_hypershell_background_job() {
        let engine = HyperShellEngine::default();
        let task_id = engine.spawn_background_task("echo 'Background Complete'").await;

        // Poll for result
        for _ in 0..50 {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            if let Some(res) = engine.get_job_result(task_id).await {
                assert!(res.success);
                assert!(res.stdout.contains("Background Complete"));
                return;
            }
        }
        panic!("Background task timed out");
    }
}
