use well_ipc::mcp::{McpServer, LeaseType, McpState};
use serde_json::json;

#[test]
fn test_mcp_tools_list() {
    let server = McpServer::new();
    let tools = server.list_tools();
    
    assert_eq!(tools.len(), 4);
    assert_eq!(tools[0].name, "well_exec_command");
    assert_eq!(tools[1].name, "well_read_buffer_block");
    assert_eq!(tools[2].name, "well_edit_file");
    assert_eq!(tools[3].name, "well_inspect_env");
}

#[test]
fn test_mcp_resources_list() {
    let server = McpServer::new();
    let resources = server.list_resources();
    
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].uri, "pane://active");
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
