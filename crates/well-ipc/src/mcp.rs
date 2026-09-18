use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[async_trait]
pub trait CommandExecutor: Send + Sync {
    async fn execute_sandboxed(&self, cmd: &str, args: &[String]) -> Result<(), String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone)]
pub enum LeaseType {
    Read,
    Write,
}

#[derive(Debug, Clone)]
pub struct MeshLock {
    pub owner: String,
    pub lease_type: LeaseType,
}

pub struct McpState {
    pub active_leases: HashMap<String, MeshLock>,
}

impl McpState {
    pub fn new() -> Self {
        Self {
            active_leases: HashMap::new(),
        }
    }

    pub fn acquire_lease(
        &mut self,
        resource_id: &str,
        owner: &str,
        lease_type: LeaseType,
    ) -> Result<(), String> {
        if let Some(existing) = self.active_leases.get(resource_id) {
            return Err(format!(
                "Resource {} is currently locked by {}",
                resource_id, existing.owner
            ));
        }
        self.active_leases.insert(
            resource_id.to_string(),
            MeshLock {
                owner: owner.to_string(),
                lease_type,
            },
        );
        Ok(())
    }

    pub fn release_lease(&mut self, resource_id: &str, owner: &str) -> Result<(), String> {
        if let Some(existing) = self.active_leases.get(resource_id) {
            if existing.owner == owner {
                self.active_leases.remove(resource_id);
                Ok(())
            } else {
                Err(format!("Cannot release lease owned by {}", existing.owner))
            }
        } else {
            Err(format!("No active lease on {}", resource_id))
        }
    }
}

pub struct McpServer {
    pub state: Arc<Mutex<McpState>>,
}

impl McpServer {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(McpState::new())),
        }
    }

    pub fn list_tools(&self) -> Vec<Tool> {
        vec![
            Tool {
                name: "well_exec_command".to_string(),
                description: "Execute a command in an isolated terminal sandbox.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": { "type": "string" },
                        "args": { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["command"]
                }),
            },
            Tool {
                name: "well_read_buffer_block".to_string(),
                description: "Read the contents of a specific terminal buffer block.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "pane_id": { "type": "string" },
                        "block_index": { "type": "integer" }
                    },
                    "required": ["pane_id", "block_index"]
                }),
            },
            Tool {
                name: "well_edit_file".to_string(),
                description: "Propose a diff edit to an active file buffer in Mneme.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "file_uri": { "type": "string" },
                        "diff": { "type": "string" }
                    },
                    "required": ["file_uri", "diff"]
                }),
            },
            Tool {
                name: "well_inspect_env".to_string(),
                description: "Inspect the environment variables of a specific pane.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "pane_id": { "type": "string" }
                    },
                    "required": ["pane_id"]
                }),
            },
        ]
    }

    pub fn list_resources(&self) -> Vec<Resource> {
        vec![Resource {
            uri: "pane://active".to_string(),
            name: "Active Terminal Pane".to_string(),
            description: Some("The currently active terminal pane buffer.".to_string()),
            mime_type: Some("text/plain".to_string()),
        }]
    }

    pub async fn call_tool(
        &self,
        name: &str,
        args: Value,
        router: &impl CommandExecutor,
    ) -> Result<String, String> {
        let mut state = self.state.lock().unwrap();

        match name {
            "well_exec_command" => {
                let cmd = args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing command argument")?;
                let mut cmd_args = Vec::new();
                if let Some(arr) = args.get("args").and_then(|v| v.as_array()) {
                    for arg in arr {
                        if let Some(s) = arg.as_str() {
                            cmd_args.push(s.to_string());
                        }
                    }
                }
                // Acquire write lease on execution subsystem for this AI agent
                state.acquire_lease("execution", "ai-agent", LeaseType::Write)?;

                // Execute using router behind MeshLock
                let res = router.execute_sandboxed(cmd, &cmd_args).await;

                state.release_lease("execution", "ai-agent")?;
                match res {
                    Ok(_) => Ok(format!("Executed command: {} {:?}", cmd, cmd_args)),
                    Err(e) => Err(format!("Execution failed: {}", e)),
                }
            }
            "well_edit_file" => {
                let file_uri = args
                    .get("file_uri")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing file_uri")?;
                let diff = args
                    .get("diff")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing diff")?;

                state.acquire_lease(file_uri, "ai-agent", LeaseType::Write)?;
                // Apply Mneme B-tree buffer edits here
                // ...
                state.release_lease(file_uri, "ai-agent")?;
                Ok(format!("Diff applied to {}", file_uri))
            }
            _ => Err("Unknown tool".to_string()),
        }
    }

    pub fn read_resource(&self, uri: &str) -> Result<String, String> {
        let mut state = self.state.lock().unwrap();
        state.acquire_lease(uri, "ai-agent", LeaseType::Read)?;

        let content = if uri == "pane://active" {
            "Simulated terminal buffer content block from Orpheus".to_string()
        } else {
            "Unknown resource".to_string()
        };

        state.release_lease(uri, "ai-agent")?;
        Ok(content)
    }
}
