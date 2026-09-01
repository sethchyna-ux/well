//! Hyprlang Configuration Engine for Well
//!
//! Provides parsing and serialization for Hyprland/Hyprlang-syntax configuration
//! files (.hl / .conf) within the Well terminal architecture.
//!
//! Supports:
//! - Block syntax: `category { key = value }`
//! - Top-level assignments: `key = value`
//! - Binding syntax: `bind = MODIFIERS, KEY, ACTION [, ARGS]`
//! - Comments (#) and whitespace trimming

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use well_ipc::TheiaConfigPayload;
use crate::KeybindingRow;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HyprlangDocument {
    pub blocks: HashMap<String, HashMap<String, String>>,
    pub variables: HashMap<String, String>,
    pub bindings: Vec<HyprlangBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HyprlangBinding {
    pub modifiers: String,
    pub key: String,
    pub dispatcher: String,
    pub argument: Option<String>,
}

#[derive(Debug, Clone)]
pub enum HyprlangParseError {
    InvalidSyntax(usize, String),
    UnmatchedBrace(usize),
}

impl std::fmt::Display for HyprlangParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSyntax(line, msg) => write!(f, "Hyprlang error at line {}: {}", line, msg),
            Self::UnmatchedBrace(line) => write!(f, "Unmatched closing brace at line {}", line),
        }
    }
}

impl std::error::Error for HyprlangParseError {}

/// Parser for Hyprlang config documents
pub struct HyprlangParser;

impl HyprlangParser {
    pub fn parse(input: &str) -> Result<HyprlangDocument, HyprlangParseError> {
        let mut blocks: HashMap<String, HashMap<String, String>> = HashMap::new();
        let mut variables: HashMap<String, String> = HashMap::new();
        let mut bindings: Vec<HyprlangBinding> = Vec::new();

        let mut current_block: Option<String> = None;

        for (line_idx, line) in input.lines().enumerate() {
            let line_num = line_idx + 1;
            let trimmed = line.trim();

            // Skip empty lines and full comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Strip inline comments
            let code_part = if let Some(pos) = trimmed.find('#') {
                trimmed[..pos].trim()
            } else {
                trimmed
            };

            if code_part.ends_with('{') {
                // Starting a block: e.g. "appearance {"
                let block_name = code_part.trim_end_matches('{').trim().to_string();
                if block_name.is_empty() {
                    return Err(HyprlangParseError::InvalidSyntax(line_num, "Empty block name".into()));
                }
                current_block = Some(block_name);
                continue;
            }

            if code_part == "}" {
                // Closing a block
                if current_block.is_none() {
                    return Err(HyprlangParseError::UnmatchedBrace(line_num));
                }
                current_block = None;
                continue;
            }

            // Handle assignments or directives
            if let Some((lhs, rhs)) = code_part.split_once('=') {
                let key = lhs.trim();
                let value = rhs.trim();

                if key.eq_ignore_ascii_case("bind") {
                    // Hyprlang binding format: bind = MOD, KEY, DISPATCHER, ARG
                    let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
                    if parts.len() >= 3 {
                        bindings.push(HyprlangBinding {
                            modifiers: parts[0].to_string(),
                            key: parts[1].to_string(),
                            dispatcher: parts[2].to_string(),
                            argument: if parts.len() > 3 { Some(parts[3..].join(", ")) } else { None },
                        });
                    }
                } else if let Some(ref block) = current_block {
                    blocks
                        .entry(block.clone())
                        .or_default()
                        .insert(key.to_string(), value.to_string());
                } else {
                    variables.insert(key.to_string(), value.to_string());
                }
            }
        }

        Ok(HyprlangDocument {
            blocks,
            variables,
            bindings,
        })
    }

    /// Converts a HyprlangDocument into a TheiaConfigPayload
    pub fn to_theia_config(doc: &HyprlangDocument) -> TheiaConfigPayload {
        let mut config = TheiaConfigPayload::default();

        if let Some(app) = doc.blocks.get("appearance") {
            if let Some(val) = app.get("theme_id").and_then(|v| v.parse::<u32>().ok()) {
                config.theme_id = val;
            }
            if let Some(val) = app.get("background_opacity").and_then(|v| v.parse::<f32>().ok()) {
                config.background_opacity = val.clamp(0.0, 1.0);
            }
            if let Some(val) = app.get("glass_blur_radius").and_then(|v| v.parse::<f32>().ok()) {
                config.glass_blur_radius = val;
            }
            if let Some(val) = app.get("font_size").and_then(|v| v.parse::<f32>().ok()) {
                config.font_size = val;
            }
            if let Some(val) = app.get("line_height").and_then(|v| v.parse::<f32>().ok()) {
                config.line_height = val;
            }
            if let Some(val) = app.get("cursor_style").and_then(|v| v.parse::<u32>().ok()) {
                config.cursor_style = val;
            }
            if let Some(val) = app.get("cursor_blink").and_then(|v| v.parse::<bool>().ok()) {
                config.cursor_blink = val;
            }
        }

        if let Some(perf) = doc.blocks.get("performance") {
            if let Some(val) = perf.get("scan_timeout_ms").and_then(|v| v.parse::<u32>().ok()) {
                config.scan_timeout_ms = val;
            }
            if let Some(val) = perf.get("command_timeout_ms").and_then(|v| v.parse::<u32>().ok()) {
                config.command_timeout_ms = val;
            }
            if let Some(val) = perf.get("enable_transient_prompt").and_then(|v| v.parse::<bool>().ok()) {
                config.enable_transient_prompt = val;
            }
            if let Some(val) = perf.get("scrollback_limit").and_then(|v| v.parse::<u32>().ok()) {
                config.scrollback_limit = val;
            }
        }

        if let Some(shader) = doc.blocks.get("crt_shader") {
            if let Some(val) = shader.get("screen_curvature").and_then(|v| v.parse::<f32>().ok()) {
                config.screen_curvature = val;
            }
            if let Some(val) = shader.get("scanline_frequency").and_then(|v| v.parse::<f32>().ok()) {
                config.scanline_frequency = val;
            }
            if let Some(val) = shader.get("glow_radius").and_then(|v| v.parse::<f32>().ok()) {
                config.glow_radius = val;
            }
        }

        config
    }

    /// Converts Hyprlang bindings to KeybindingRow items
    pub fn to_keybinding_rows(doc: &HyprlangDocument) -> Vec<KeybindingRow> {
        doc.bindings
            .iter()
            .map(|b| {
                let chord = format!("{}+{}", b.modifiers, b.key);
                let action = if let Some(ref arg) = b.argument {
                    format!("{}: {}", b.dispatcher, arg)
                } else {
                    b.dispatcher.clone()
                };
                KeybindingRow {
                    chord,
                    action,
                    conflict: false,
                }
            })
            .collect()
    }
}

/// Serializer for emitting Well configuration in Hyprlang (.hl) format
pub struct HyprlangEmitter;

impl HyprlangEmitter {
    pub fn emit(config: &TheiaConfigPayload, keybindings: &[KeybindingRow]) -> String {
        let mut out = String::new();

        out.push_str("# Well (Phrear) Terminal Configuration\n");
        out.push_str("# Hyprlang Format Specification (.hl)\n\n");

        out.push_str("appearance {\n");
        out.push_str(&format!("    theme_id = {}\n", config.theme_id));
        out.push_str(&format!("    background_opacity = {:.2}\n", config.background_opacity));
        out.push_str(&format!("    glass_blur_radius = {:.1}\n", config.glass_blur_radius));
        out.push_str(&format!("    font_size = {:.1}\n", config.font_size));
        out.push_str(&format!("    line_height = {:.2}\n", config.line_height));
        out.push_str(&format!("    cursor_style = {}\n", config.cursor_style));
        out.push_str(&format!("    cursor_blink = {}\n", config.cursor_blink));
        out.push_str("}\n\n");

        out.push_str("performance {\n");
        out.push_str(&format!("    scan_timeout_ms = {}\n", config.scan_timeout_ms));
        out.push_str(&format!("    command_timeout_ms = {}\n", config.command_timeout_ms));
        out.push_str(&format!("    enable_transient_prompt = {}\n", config.enable_transient_prompt));
        out.push_str(&format!("    scrollback_limit = {}\n", config.scrollback_limit));
        out.push_str("}\n\n");

        out.push_str("crt_shader {\n");
        out.push_str(&format!("    screen_curvature = {:.2}\n", config.screen_curvature));
        out.push_str(&format!("    scanline_frequency = {:.2}\n", config.scanline_frequency));
        out.push_str(&format!("    glow_radius = {:.2}\n", config.glow_radius));
        out.push_str("}\n\n");

        if !keybindings.is_empty() {
            out.push_str("# Keybindings\n");
            for kb in keybindings {
                if let Some((mods, key)) = kb.chord.split_once('+') {
                    out.push_str(&format!("bind = {}, {}, {}\n", mods, key, kb.action));
                } else {
                    out.push_str(&format!("bind = , {}, {}\n", kb.chord, kb.action));
                }
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hyprlang_blocks() {
        let input = r#"
        # Sample well.hl config
        appearance {
            theme_id = 2
            background_opacity = 0.88
            glass_blur_radius = 18.5
            font_size = 15.0
            cursor_style = 1
            cursor_blink = false
        }

        performance {
            scan_timeout_ms = 40
            command_timeout_ms = 450
            enable_transient_prompt = true
            scrollback_limit = 50000
        }

        bind = SUPER, Return, exec, fish
        bind = CTRL_SHIFT, T, new_tab
        "#;

        let doc = HyprlangParser::parse(input).expect("Failed to parse Hyprlang input");
        assert_eq!(doc.blocks.len(), 2);
        assert_eq!(doc.bindings.len(), 2);

        let config = HyprlangParser::to_theia_config(&doc);
        assert_eq!(config.theme_id, 2);
        assert_eq!(config.background_opacity, 0.88);
        assert_eq!(config.glass_blur_radius, 18.5);
        assert_eq!(config.font_size, 15.0);
        assert_eq!(config.cursor_style, 1);
        assert!(!config.cursor_blink);
        assert_eq!(config.scan_timeout_ms, 40);
        assert_eq!(config.command_timeout_ms, 450);
        assert!(config.enable_transient_prompt);
        assert_eq!(config.scrollback_limit, 50000);

        let rows = HyprlangParser::to_keybinding_rows(&doc);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].chord, "SUPER+Return");
        assert_eq!(rows[0].action, "exec: fish");
    }

    #[test]
    fn test_hyprlang_round_trip() {
        let mut original = TheiaConfigPayload::default();
        original.theme_id = 3;
        original.background_opacity = 0.92;
        original.font_size = 16.0;

        let bindings = vec![
            KeybindingRow {
                chord: "SUPER+Return".to_string(),
                action: "exec: fish".to_string(),
                conflict: false,
            },
        ];

        let emitted = HyprlangEmitter::emit(&original, &bindings);
        let parsed_doc = HyprlangParser::parse(&emitted).expect("Failed to parse emitted Hyprlang");
        let restored = HyprlangParser::to_theia_config(&parsed_doc);

        assert_eq!(restored.theme_id, original.theme_id);
        assert!((restored.background_opacity - original.background_opacity).abs() < 0.01);
        assert_eq!(restored.font_size, original.font_size);
    }
}
