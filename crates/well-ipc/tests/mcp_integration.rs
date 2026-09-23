use async_trait::async_trait;
use well_ipc::mcp::{CommandExecutor, LeaseType, McpServer, McpState};

#[test]
fn test_mcp_tools_list() {
    let server = McpServer::new();
    let tools = server.list_tools();

    assert_eq!(tools.len(), 2);
    assert!(tools
        .iter()
        .any(|tool| tool.name == "well_runtime_capabilities"));
    assert!(tools.iter().any(|tool| tool.name == "well_exec_command"));
}

#[test]
fn test_mcp_resources_list() {
    let server = McpServer::new();
    let resources = server.list_resources();

    assert!(resources.is_empty());
}

struct NoopExecutor;

#[async_trait]
impl CommandExecutor for NoopExecutor {
    async fn execute_sandboxed(&self, _cmd: &str, _args: &[String]) -> Result<(), String> {
        Ok(())
    }
}

#[tokio::test]
async fn test_mcp_runtime_capabilities_reports_honest_boundaries() {
    let server = McpServer::new();
    let response = server
        .call_tool(
            "well_runtime_capabilities",
            serde_json::json!({}),
            &NoopExecutor,
        )
        .await
        .expect("capability tool should succeed");
    let response: serde_json::Value =
        serde_json::from_str(&response).expect("capabilities should be JSON");

    assert_eq!(response["protocol"], serde_json::json!("caduceus/1.0"));
    assert_eq!(
        response["gpu"]["remote_gpu_control"],
        serde_json::json!(false)
    );
    assert!(response["active_methods"]
        .as_array()
        .expect("active methods array")
        .iter()
        .any(|method| method["method"] == serde_json::json!("shell.input")));
    assert!(response["unsupported_methods"]
        .as_array()
        .expect("unsupported methods array")
        .iter()
        .any(|method| method["method"] == serde_json::json!("profile.save")));
}

#[tokio::test]
async fn test_mcp_mneme_edit_file_is_explicitly_unsupported() {
    let server = McpServer::new();
    let err = server
        .call_tool(
            "well_edit_file",
            serde_json::json!({
                "file_uri": "file:///tmp/example.rs",
                "diff": "@@"
            }),
            &NoopExecutor,
        )
        .await
        .expect_err("Mneme runtime edit surface should not fake success");

    assert!(err.contains("Mneme editor diff application is not exposed"));
}

#[test]
fn test_mesh_lock_leasing() {
    let mut state = McpState::new();

    // Acquire a lock
    let res = state.acquire_lease("pane_1", "agent_a", LeaseType::Write);
    assert!(res.is_ok());

    // Attempt double acquire
    let res = state.acquire_lease("pane_1", "agent_b", LeaseType::Read);
    assert!(res.is_err());

    // Release lock with wrong owner
    let res = state.release_lease("pane_1", "agent_b");
    assert!(res.is_err());

    // Release lock properly
    let res = state.release_lease("pane_1", "agent_a");
    assert!(res.is_ok());

    // Second acquire should now succeed
    let res = state.acquire_lease("pane_1", "agent_b", LeaseType::Read);
    assert!(res.is_ok());
}
