//! well-wasm: Sandboxed WebAssembly Extension Engine for Well Terminal
//!
//! Provides a safe, isolated runtime environment for third-party terminal plugins,
//! prompt decorators (`well_hook_prompt_filter`), and status-bar badges (`well_hook_status_badge`)
//! using deterministic Wasm execution (`wasmi`).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use wasmi::{Caller, Engine, Func, Linker, Memory, MemoryType, Module, Store};

/// Metadata descriptor for a sandboxed terminal extension
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDescriptor {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
}

#[derive(Clone)]
struct LoadedPlugin {
    descriptor: PluginDescriptor,
    wasm_bytes: Vec<u8>,
}

/// Sandboxed WebAssembly execution host
#[derive(Default)]
pub struct WasmPluginHost {
    engine: Engine,
    loaded_plugins: HashMap<String, PluginDescriptor>,
    plugin_binaries: HashMap<String, LoadedPlugin>,
}

impl WasmPluginHost {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the default user plugin directory (`~/.config/well/plugins`).
    pub fn default_plugin_dir() -> PathBuf {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".config")
            .join("well")
            .join("plugins")
    }

    /// Scans a directory for `*.wasm` plugin modules, validates each module,
    /// and registers valid plugins while isolating corrupt or invalid files.
    pub fn scan_plugin_directory(&mut self, dir: &Path) -> Result<usize, String> {
        if !dir.exists() {
            return Ok(0);
        }
        let entries = fs::read_dir(dir)
            .map_err(|e| format!("Failed to read plugin directory {}: {}", dir.display(), e))?;
        let mut count = 0usize;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                if let Ok(bytes) = fs::read(&path) {
                    if self.validate_wasm(&bytes).is_ok() {
                        let stem = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("plugin")
                            .to_string();
                        let desc = PluginDescriptor {
                            name: stem.clone(),
                            version: "1.0.0".to_string(),
                            author: "User Plugin".to_string(),
                            description: format!("Loaded from {}", path.display()),
                        };
                        self.register_plugin_binary(desc, bytes);
                        count += 1;
                    }
                }
            }
        }
        Ok(count)
    }

    /// Registers plugin metadata
    pub fn register_metadata(&mut self, descriptor: PluginDescriptor) {
        self.loaded_plugins
            .insert(descriptor.name.clone(), descriptor);
    }

    /// Registers both plugin metadata and its validated Wasm binary payload
    pub fn register_plugin_binary(&mut self, descriptor: PluginDescriptor, wasm_bytes: Vec<u8>) {
        let name = descriptor.name.clone();
        self.loaded_plugins.insert(name.clone(), descriptor.clone());
        self.plugin_binaries.insert(
            name,
            LoadedPlugin {
                descriptor,
                wasm_bytes,
            },
        );
    }

    /// Validates and compiles raw WebAssembly bytecode
    pub fn validate_wasm(&self, wasm_bytes: &[u8]) -> Result<(), String> {
        Module::new(&self.engine, wasm_bytes)
            .map_err(|e| format!("Invalid WebAssembly module: {}", e))?;
        Ok(())
    }

    /// Executes an integer transformation hook in a sandboxed Wasm instance
    pub fn execute_int_hook(
        &self,
        wasm_bytes: &[u8],
        func_name: &str,
        input: i32,
    ) -> Result<i32, String> {
        let module = Module::new(&self.engine, wasm_bytes)
            .map_err(|e| format!("Wasm compile error: {}", e))?;

        let mut store = Store::new(&self.engine, ());
        let mut linker = Linker::new(&self.engine);

        linker
            .define(
                "host",
                "log_i32",
                Func::wrap(&mut store, |_caller: Caller<'_, ()>, val: i32| {
                    let _ = val;
                }),
            )
            .map_err(|e| format!("Linker error: {}", e))?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| format!("Instantiation error: {}", e))?
            .start(&mut store)
            .map_err(|e| format!("Start error: {}", e))?;

        let func = instance
            .get_typed_func::<i32, i32>(&store, func_name)
            .map_err(|e| format!("Exported function '{}' not found: {}", func_name, e))?;

        func.call(&mut store, input)
            .map_err(|e| format!("Execution error in '{}': {}", func_name, e))
    }

    /// Executes `well_hook_status_badge(exit_code: i32) -> i32` or memory badge hook.
    /// If the Wasm module exports `well_hook_status_badge(i32) -> i32`, maps the returned
    /// badge code and plugin name into a formatted status-bar badge string.
    /// Any Wasm trap (e.g. `unreachable` or division by zero) is safely caught and returned as `Err`.
    pub fn well_hook_status_badge(
        &self,
        plugin_name: &str,
        wasm_bytes: &[u8],
        exit_code: i32,
    ) -> Result<String, String> {
        let code = self.execute_int_hook(wasm_bytes, "well_hook_status_badge", exit_code)?;
        Ok(format!("[{plugin_name}:{code}]"))
    }

    /// Executes `well_hook_prompt_filter(prompt_len: i32) -> i32` in the Wasm sandbox.
    /// Any trap or missing export is caught without affecting the host terminal process.
    pub fn well_hook_prompt_filter(
        &self,
        wasm_bytes: &[u8],
        prompt: &str,
    ) -> Result<String, String> {
        let module = Module::new(&self.engine, wasm_bytes)
            .map_err(|e| format!("Wasm compile error: {}", e))?;
        let mut store = Store::new(&self.engine, ());
        let mut linker = Linker::new(&self.engine);
        let _ = Memory::new(&mut store, MemoryType::new(1, Some(4)).map_err(|e| e.to_string())?);

        linker
            .define(
                "host",
                "log_i32",
                Func::wrap(&mut store, |_caller: Caller<'_, ()>, val: i32| {
                    let _ = val;
                }),
            )
            .map_err(|e| format!("Linker error: {}", e))?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| format!("Instantiation error: {}", e))?
            .start(&mut store)
            .map_err(|e| format!("Start error: {}", e))?;

        let func = instance
            .get_typed_func::<i32, i32>(&store, "well_hook_prompt_filter")
            .map_err(|e| format!("Exported function 'well_hook_prompt_filter' not found: {}", e))?;

        let tag = func
            .call(&mut store, prompt.len() as i32)
            .map_err(|e| format!("Trap in 'well_hook_prompt_filter': {}", e))?;

        Ok(format!("{prompt} (wasm:{tag})"))
    }

    /// Evaluates `well_hook_status_badge` across all loaded plugins, isolating any plugin that traps.
    pub fn collect_status_badges(&self, exit_code: i32) -> Vec<String> {
        let mut badges = Vec::new();
        let mut names: Vec<&String> = self.plugin_binaries.keys().collect();
        names.sort();
        for name in names {
            if let Some(plugin) = self.plugin_binaries.get(name) {
                if let Ok(badge) = self.well_hook_status_badge(
                    &plugin.descriptor.name,
                    &plugin.wasm_bytes,
                    exit_code,
                ) {
                    badges.push(badge);
                }
            }
        }
        badges
    }

    /// Returns all registered plugins
    pub fn list_plugins(&self) -> Vec<&PluginDescriptor> {
        self.loaded_plugins.values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal valid WebAssembly binary:
    // (module
    //   (func (export "add_forty_two") (param i32) (result i32)
    //     local.get 0
    //     i32.const 42
    //     i32.add))
    const SAMPLE_WASM_BYTES: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // \0asm v1
        0x01, 0x06, 0x01, 0x60, 0x01, 0x7f, 0x01, 0x7f, // Type section: (i32) -> i32
        0x03, 0x02, 0x01, 0x00, // Function section: func 0 uses type 0
        0x07, 0x11, 0x01, 0x0d, // Export section: len 17, 1 export, name len 13
        0x61, 0x64, 0x64, 0x5f, 0x66, 0x6f, 0x72, 0x74, 0x79, 0x5f, 0x74, 0x77,
        0x6f, // "add_forty_two"
        0x00, 0x00, // kind: function (0), index: 0
        0x0a, 0x09, 0x01, 0x07, 0x00, // Code section: len 9, 1 func, size 7, 0 locals
        0x20, 0x00, // local.get 0
        0x41, 0x2a, // i32.const 42
        0x6a, // i32.add
        0x0b, // end
    ];

    // Minimal valid WebAssembly binary exporting `well_hook_status_badge`:
    // (module
    //   (func (export "well_hook_status_badge") (param i32) (result i32)
    //     local.get 0
    //     i32.const 100
    //     i32.add))
    pub const BADGE_WASM_BYTES: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // \0asm v1
        0x01, 0x06, 0x01, 0x60, 0x01, 0x7f, 0x01, 0x7f, // Type: (i32) -> i32
        0x03, 0x02, 0x01, 0x00, // Func 0
        0x07, 0x1a, 0x01, 0x16, // Export section: len 26, 1 export, name len 22
        b'w', b'e', b'l', b'l', b'_', b'h', b'o', b'o', b'k', b'_', b's', b't', b'a', b't', b'u',
        b's', b'_', b'b', b'a', b'd', b'g', b'e', 0x00, 0x00, // func 0
        0x0a, 0x0a, 0x01, 0x08, 0x00, // Code section: len 10, 1 func, body len 8, 0 locals
        0x20, 0x00, // local.get 0
        0x41, 0xe4, 0x00, // i32.const 100 (LEB128)
        0x6a, // i32.add
        0x0b, // end
    ];

    // Trapping WebAssembly binary exporting `well_hook_status_badge` that executes `unreachable` (0x00):
    pub const TRAPPING_WASM_BYTES: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // \0asm v1
        0x01, 0x06, 0x01, 0x60, 0x01, 0x7f, 0x01, 0x7f, // Type: (i32) -> i32
        0x03, 0x02, 0x01, 0x00, // Func 0
        0x07, 0x1a, 0x01, 0x16, // Export section: name len 22
        b'w', b'e', b'l', b'l', b'_', b'h', b'o', b'o', b'k', b'_', b's', b't', b'a', b't', b'u',
        b's', b'_', b'b', b'a', b'd', b'g', b'e', 0x00, 0x00, 0x0a, 0x05, 0x01, 0x03,
        0x00, // Code section: body len 3, 0 locals
        0x00, // unreachable trap!
        0x0b, // end
    ];

    #[test]
    fn test_wasm_validation() {
        let host = WasmPluginHost::new();
        assert!(host.validate_wasm(SAMPLE_WASM_BYTES).is_ok());

        let invalid_bytes = [0x00, 0x01, 0x02, 0x03];
        assert!(host.validate_wasm(&invalid_bytes).is_err());
    }

    #[test]
    fn test_wasm_plugin_execution() {
        let mut host = WasmPluginHost::new();
        let desc = PluginDescriptor {
            name: "sample-math-plugin".to_string(),
            version: "1.0.0".to_string(),
            author: "Well Engineering".to_string(),
            description: "Sample sandboxed Wasm extension".to_string(),
        };
        host.register_metadata(desc);
        assert_eq!(host.list_plugins().len(), 1);

        let result = host
            .execute_int_hook(SAMPLE_WASM_BYTES, "add_forty_two", 8)
            .expect("Wasm execution should succeed");
        assert_eq!(result, 50); // 8 + 42 = 50
    }

    #[test]
    fn test_status_badge_and_fault_isolation_on_trap() {
        let mut host = WasmPluginHost::new();
        host.register_plugin_binary(
            PluginDescriptor {
                name: "good-badge".to_string(),
                version: "1.0.0".to_string(),
                author: "Well".to_string(),
                description: "Healthy status badge".to_string(),
            },
            BADGE_WASM_BYTES.to_vec(),
        );
        host.register_plugin_binary(
            PluginDescriptor {
                name: "faulty-trap".to_string(),
                version: "1.0.0".to_string(),
                author: "Well".to_string(),
                description: "Trapping status badge".to_string(),
            },
            TRAPPING_WASM_BYTES.to_vec(),
        );

        // Direct call to trapping module returns Err without crashing host
        let trap_res = host.well_hook_status_badge("faulty-trap", TRAPPING_WASM_BYTES, 0);
        assert!(trap_res.is_err());

        // Collecting badges across all loaded plugins isolates the trap and returns the healthy badge
        let badges = host.collect_status_badges(7);
        assert_eq!(badges, vec!["[good-badge:107]".to_string()]);
    }
}
