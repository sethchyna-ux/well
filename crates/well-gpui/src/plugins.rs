//! GPUI Wasm Extension Host Bridge (`well-wasm` integration).
//!
//! Loads user plugins from `~/.config/well/plugins/*.wasm` and evaluates status-bar
//! badges and prompt filters with strict sandbox fault isolation.

use std::path::Path;
use well_wasm::{PluginDescriptor, WasmPluginHost};

#[allow(dead_code)]
pub struct GpuiPluginRuntime {
    host: WasmPluginHost,
}

impl Default for GpuiPluginRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl GpuiPluginRuntime {
    pub fn new() -> Self {
        let mut host = WasmPluginHost::new();
        let default_dir = WasmPluginHost::default_plugin_dir();
        let _ = host.scan_plugin_directory(&default_dir);
        Self { host }
    }

    pub fn from_directory(dir: &Path) -> Result<Self, String> {
        let mut host = WasmPluginHost::new();
        host.scan_plugin_directory(dir)?;
        Ok(Self { host })
    }

    pub fn register_plugin(&mut self, descriptor: PluginDescriptor, wasm_bytes: Vec<u8>) {
        self.host.register_plugin_binary(descriptor, wasm_bytes);
    }

    pub fn status_bar_badges(&self, last_exit_code: i32) -> Vec<String> {
        self.host.collect_status_badges(last_exit_code)
    }

    pub fn plugin_count(&self) -> usize {
        self.host.list_plugins().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BADGE_WASM: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x60, 0x01, 0x7f, 0x01,
        0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x1a, 0x01, 0x16, b'w', b'e', b'l', b'l', b'_', b'h',
        b'o', b'o', b'k', b'_', b's', b't', b'a', b't', b'u', b's', b'_', b'b', b'a', b'd', b'g',
        b'e', 0x00, 0x00, 0x0a, 0x0a, 0x01, 0x08, 0x00, 0x20, 0x00, 0x41, 0xe4, 0x00, 0x6a, 0x0b,
    ];

    const TRAP_WASM: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x60, 0x01, 0x7f, 0x01,
        0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x1a, 0x01, 0x16, b'w', b'e', b'l', b'l', b'_', b'h',
        b'o', b'o', b'k', b'_', b's', b't', b'a', b't', b'u', b's', b'_', b'b', b'a', b'd', b'g',
        b'e', 0x00, 0x00, 0x0a, 0x05, 0x01, 0x03, 0x00, 0x00, 0x0b,
    ];

    #[test]
    fn gpui_plugin_runtime_renders_badges_and_isolates_wasm_traps() {
        let mut runtime = GpuiPluginRuntime {
            host: WasmPluginHost::new(),
        };
        runtime.register_plugin(
            PluginDescriptor {
                name: "telemetry".to_string(),
                version: "1.0.0".to_string(),
                author: "Well".to_string(),
                description: "Status bar badge".to_string(),
            },
            BADGE_WASM.to_vec(),
        );
        runtime.register_plugin(
            PluginDescriptor {
                name: "panicking-plugin".to_string(),
                version: "0.1.0".to_string(),
                author: "Untrusted".to_string(),
                description: "Executes unreachable trap".to_string(),
            },
            TRAP_WASM.to_vec(),
        );

        assert_eq!(runtime.plugin_count(), 2);
        let badges = runtime.status_bar_badges(0);
        assert_eq!(badges, vec!["[telemetry:100]".to_string()]);
    }
}
