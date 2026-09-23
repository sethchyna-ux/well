//! well-wasm: Sandboxed WebAssembly Extension Engine for Well Terminal
//!
//! Provides a safe, isolated runtime environment for third-party terminal plugins,
//! prompt decorators, and command transformation hooks using deterministic Wasm execution.

use std::collections::HashMap;
use wasmi::{Caller, Engine, Func, Linker, Module, Store};

/// Metadata descriptor for a sandboxed terminal extension
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDescriptor {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
}

/// Sandboxed WebAssembly execution host
#[derive(Default)]
pub struct WasmPluginHost {
    engine: Engine,
    loaded_plugins: HashMap<String, PluginDescriptor>,
}

impl WasmPluginHost {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers plugin metadata
    pub fn register_metadata(&mut self, descriptor: PluginDescriptor) {
        self.loaded_plugins
            .insert(descriptor.name.clone(), descriptor);
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

        // Host import: log an integer from inside Wasm
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
}
