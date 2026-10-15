pub const BACKEND: &str = "wgpu";

pub fn cube_count(workspaces: usize) -> usize {
    workspaces.max(1)
}
