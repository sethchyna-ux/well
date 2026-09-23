use crate::caduceus_capabilities;
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

#[derive(Default)]
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

#[derive(Default)]
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
                name: "well_runtime_capabilities".to_string(),
                description: "Return the active Well desktop automation capabilities and unsupported surfaces.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            },
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
        ]
    }

    pub fn list_resources(&self) -> Vec<Resource> {
        Vec::new()
    }

    pub async fn call_tool(
        &self,
        name: &str,
        args: Value,
        router: &impl CommandExecutor,
    ) -> Result<String, String> {
        match name {
            "well_runtime_capabilities" => Ok(caduceus_capabilities().to_string()),
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
                {
                    let mut state = self.state.lock().unwrap();
                    state.acquire_lease("execution", "ai-agent", LeaseType::Write)?;
                }

                // Execute using router behind MeshLock
                let res = router.execute_sandboxed(cmd, &cmd_args).await;

                {
                    let mut state = self.state.lock().unwrap();
                    state.release_lease("execution", "ai-agent")?;
                }
                match res {
                    Ok(_) => Ok(format!("Executed command: {} {:?}", cmd, cmd_args)),
                    Err(e) => Err(format!("Execution failed: {}", e)),
                }
            }
            "well_edit_file" => Err(
                "Mneme editor diff application is not exposed by the current desktop runtime."
                    .to_string(),
            ),
            _ => Err("Unknown tool".to_string()),
        }
    }

    pub fn read_resource(&self, uri: &str) -> Result<String, String> {
        Err(format!(
            "MCP resource {uri} is not exposed by the current desktop runtime."
        ))
    }
}
