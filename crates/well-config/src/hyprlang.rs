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

use crate::KeybindingRow;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use well_ipc::TheiaConfigPayload;

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
    UnclosedBlock(String),
}

impl std::fmt::Display for HyprlangParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSyntax(line, msg) => write!(f, "Hyprlang error at line {}: {}", line, msg),
            Self::UnmatchedBrace(line) => write!(f, "Unmatched closing brace at line {}", line),
            Self::UnclosedBlock(block) => write!(f, "Unclosed Hyprlang block {block:?}"),
        }
    }
}

impl std::error::Error for HyprlangParseError {}

fn strip_inline_comment(line: &str) -> &str {
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        if character == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }

        match (quote, character) {
            (Some(active), current) if active == current => quote = None,
            (None, '\'' | '"') => quote = Some(character),
            (None, '#') => return line[..index].trim_end(),
            _ => {}
        }
    }

    line
}

fn decode_value(value: &str) -> String {
    let trimmed = value.trim();
    let mut characters = trimmed.chars();
    let Some(quote) = characters.next() else {
        return String::new();
    };

    if !matches!(quote, '\'' | '"') || !trimmed.ends_with(quote) || trimmed.len() < 2 {
        return trimmed.to_string();
    }

    let inner = &trimmed[quote.len_utf8()..trimmed.len() - quote.len_utf8()];
    let mut decoded = String::with_capacity(inner.len());
    let mut escaped = false;

    for character in inner.chars() {
        if escaped {
            match character {
                'n' => decoded.push('\n'),
                'r' => decoded.push('\r'),
                't' => decoded.push('\t'),
                '\\' => decoded.push('\\'),
                '\'' if quote == '\'' => decoded.push('\''),
                '"' if quote == '"' => decoded.push('"'),
                other => {
                    decoded.push('\\');
                    decoded.push(other);
                }
            }
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            decoded.push(character);
        }
    }

    if escaped {
        decoded.push('\\');
    }

    decoded
}

fn encode_value(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len() + 2);
    encoded.push('"');
    for character in value.chars() {
        match character {
            '\n' => encoded.push_str("\\n"),
            '\r' => encoded.push_str("\\r"),
            '\t' => encoded.push_str("\\t"),
            '\\' => encoded.push_str("\\\\"),
            '"' => encoded.push_str("\\\""),
            other => encoded.push(other),
        }
    }
    encoded.push('"');
    encoded
}

fn split_binding_fields(value: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut start = 0;
    let mut quote = None;
    let mut escaped = false;

    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        if character == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }

        match (quote, character) {
            (Some(active), current) if active == current => quote = None,
            (None, '\'' | '"') => quote = Some(character),
            (None, ',') => {
                fields.push(decode_value(&value[start..index]));
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }

    fields.push(decode_value(&value[start..]));
    fields
}

fn block_value<'a>(doc: &'a HyprlangDocument, block: &str, key: &str) -> Option<&'a str> {
    doc.blocks
        .get(block)
        .and_then(|values| values.get(key))
        .or_else(|| doc.variables.get(key))
        .map(String::as_str)
}

fn parsed_value<T: std::str::FromStr>(doc: &HyprlangDocument, block: &str, key: &str) -> Option<T> {
    block_value(doc, block, key).and_then(|value| decode_value(value).parse().ok())
}

fn string_value(doc: &HyprlangDocument, block: &str, key: &str) -> Option<String> {
    block_value(doc, block, key).map(decode_value)
}

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

            // Strip inline comments while retaining hashes inside quoted values.
            let code_part = strip_inline_comment(trimmed).trim();

            if code_part.is_empty() {
                continue;
            }

            if code_part.ends_with('{') {
                // Starting a block: e.g. "appearance {"
                let block_name = code_part.trim_end_matches('{').trim().to_string();
                if block_name.is_empty() {
                    return Err(HyprlangParseError::InvalidSyntax(
                        line_num,
                        "Empty block name".into(),
                    ));
                }
                if let Some(open_block) = current_block.as_deref() {
                    return Err(HyprlangParseError::InvalidSyntax(
                        line_num,
                        format!("Nested block {block_name:?} inside {open_block:?}"),
                    ));
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
                    let parts = split_binding_fields(value);
                    if parts.len() >= 3 {
                        bindings.push(HyprlangBinding {
                            modifiers: parts[0].clone(),
                            key: parts[1].clone(),
                            dispatcher: parts[2].clone(),
                            argument: if parts.len() > 3 {
                                Some(parts[3..].join(", "))
                            } else {
                                None
                            },
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

        if let Some(block) = current_block {
            return Err(HyprlangParseError::UnclosedBlock(block));
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

        if let Some(value) = parsed_value(doc, "appearance", "theme_id") {
            config.theme_id = value;
        }
        if let Some(value) = parsed_value::<f32>(doc, "appearance", "background_opacity") {
            config.background_opacity = value.clamp(0.0, 1.0);
        }
        if let Some(value) = parsed_value(doc, "appearance", "glass_blur_radius") {
            config.glass_blur_radius = value;
        }
        if let Some(value) = parsed_value(doc, "appearance", "font_size") {
            config.font_size = value;
        }
        if let Some(value) = parsed_value(doc, "appearance", "line_height") {
            config.line_height = value;
        }
        if let Some(value) = string_value(doc, "appearance", "font_name") {
            config.font_name = value;
        }
        if let Some(value) = parsed_value(doc, "appearance", "cursor_style") {
            config.cursor_style = value;
        }
        if let Some(value) = parsed_value(doc, "appearance", "cursor_blink") {
            config.cursor_blink = value;
        }

        if let Some(value) = parsed_value(doc, "performance", "scan_timeout_ms") {
            config.scan_timeout_ms = value;
        }
        if let Some(value) = parsed_value(doc, "performance", "command_timeout_ms") {
            config.command_timeout_ms = value;
        }
        if let Some(value) = parsed_value(doc, "performance", "enable_transient_prompt") {
            config.enable_transient_prompt = value;
        }
        if let Some(value) = parsed_value(doc, "performance", "enable_kitty_keyboard") {
            config.enable_kitty_keyboard = value;
        }
        if let Some(value) = parsed_value(doc, "performance", "scrollback_limit") {
            config.scrollback_limit = value;
        }

        if let Some(value) = parsed_value(doc, "crt_shader", "shader_preset") {
            config.shader_preset = value;
        }
        if let Some(value) = parsed_value(doc, "crt_shader", "screen_curvature") {
            config.screen_curvature = value;
        }
        if let Some(value) = parsed_value(doc, "crt_shader", "scanline_frequency") {
            config.scanline_frequency = value;
        }
        if let Some(value) = parsed_value(doc, "crt_shader", "glow_radius") {
            config.glow_radius = value;
        }

        if let Some(value) = block_value(doc, "profile", "profile_name") {
            config.profile_name = if value.trim() == "null" {
                None
            } else {
                Some(decode_value(value))
            };
        }
        if let Some(value) = string_value(doc, "profile", "prompt_preset") {
            config.prompt_preset = value;
        }
        if let Some(value) = string_value(doc, "profile", "shell_path") {
            config.shell_path = value;
        }

        if let Some(value) = string_value(doc, "ai", "pythia_provider") {
            config.pythia_provider = value;
        }
        if let Some(value) = string_value(doc, "ai", "hf_model") {
            config.hf_model = value;
        }
        if let Some(value) = string_value(doc, "ai", "gemini_model") {
            config.gemini_model = value;
        }
        if let Some(value) = string_value(doc, "ai", "ollama_url") {
            config.ollama_url = value;
        }
        if let Some(value) = string_value(doc, "ai", "ollama_model") {
            config.ollama_model = value;
        }

        config
    }

    /// Converts Hyprlang bindings to KeybindingRow items
    pub fn to_keybinding_rows(doc: &HyprlangDocument) -> Vec<KeybindingRow> {
        doc.bindings
            .iter()
            .map(|b| {
                let chord = if b.modifiers.is_empty() {
                    b.key.clone()
                } else {
                    format!("{}+{}", b.modifiers, b.key)
                };
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
        out.push_str(&format!(
            "    background_opacity = {}\n",
            config.background_opacity
        ));
        out.push_str(&format!(
            "    glass_blur_radius = {}\n",
            config.glass_blur_radius
        ));
        out.push_str(&format!("    font_size = {}\n", config.font_size));
        out.push_str(&format!("    line_height = {}\n", config.line_height));
        out.push_str(&format!(
            "    font_name = {}\n",
            encode_value(&config.font_name)
        ));
        out.push_str(&format!("    cursor_style = {}\n", config.cursor_style));
        out.push_str(&format!("    cursor_blink = {}\n", config.cursor_blink));
        out.push_str("}\n\n");

        out.push_str("performance {\n");
        out.push_str(&format!(
            "    scan_timeout_ms = {}\n",
            config.scan_timeout_ms
        ));
        out.push_str(&format!(
            "    command_timeout_ms = {}\n",
            config.command_timeout_ms
        ));
        out.push_str(&format!(
            "    enable_transient_prompt = {}\n",
            config.enable_transient_prompt
        ));
        out.push_str(&format!(
            "    enable_kitty_keyboard = {}\n",
            config.enable_kitty_keyboard
        ));
        out.push_str(&format!(
            "    scrollback_limit = {}\n",
            config.scrollback_limit
        ));
        out.push_str("}\n\n");

        out.push_str("crt_shader {\n");
        out.push_str(&format!("    shader_preset = {}\n", config.shader_preset));
        out.push_str(&format!(
            "    screen_curvature = {}\n",
            config.screen_curvature
        ));
        out.push_str(&format!(
            "    scanline_frequency = {}\n",
            config.scanline_frequency
        ));
        out.push_str(&format!("    glow_radius = {}\n", config.glow_radius));
        out.push_str("}\n\n");

        out.push_str("profile {\n");
        let profile_name = config
            .profile_name
            .as_deref()
            .map_or_else(|| "null".to_string(), encode_value);
        out.push_str(&format!("    profile_name = {profile_name}\n"));
        out.push_str(&format!(
            "    prompt_preset = {}\n",
            encode_value(&config.prompt_preset)
        ));
        out.push_str(&format!(
            "    shell_path = {}\n",
            encode_value(&config.shell_path)
        ));
        out.push_str("}\n\n");

        out.push_str("ai {\n");
        out.push_str(&format!(
            "    pythia_provider = {}\n",
            encode_value(&config.pythia_provider)
        ));
        out.push_str(&format!(
            "    hf_model = {}\n",
            encode_value(&config.hf_model)
        ));
        out.push_str(&format!(
            "    gemini_model = {}\n",
            encode_value(&config.gemini_model)
        ));
        out.push_str(&format!(
            "    ollama_url = {}\n",
            encode_value(&config.ollama_url)
        ));
        out.push_str(&format!(
            "    ollama_model = {}\n",
            encode_value(&config.ollama_model)
        ));
        out.push_str("}\n\n");

        if !keybindings.is_empty() {
            out.push_str("# Keybindings\n");
            for kb in keybindings {
                let (modifiers, key) = kb.chord.rsplit_once('+').unwrap_or(("", &kb.chord));
                let (dispatcher, argument) = kb
                    .action
                    .split_once(": ")
                    .map_or((kb.action.as_str(), None), |(dispatcher, argument)| {
                        (dispatcher, Some(argument))
                    });
                out.push_str(&format!(
                    "bind = {}, {}, {}",
                    encode_value(modifiers),
                    encode_value(key),
                    encode_value(dispatcher)
                ));
                if let Some(argument) = argument {
                    out.push_str(&format!(", {}", encode_value(argument)));
                }
                out.push('\n');
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_non_secret_fields_eq(actual: &TheiaConfigPayload, expected: &TheiaConfigPayload) {
        assert_eq!(actual.theme_id, expected.theme_id);
        assert_eq!(actual.background_opacity, expected.background_opacity);
        assert_eq!(actual.glass_blur_radius, expected.glass_blur_radius);
        assert_eq!(actual.scan_timeout_ms, expected.scan_timeout_ms);
        assert_eq!(actual.command_timeout_ms, expected.command_timeout_ms);
        assert_eq!(
            actual.enable_transient_prompt,
            expected.enable_transient_prompt
        );
        assert_eq!(actual.enable_kitty_keyboard, expected.enable_kitty_keyboard);
        assert_eq!(actual.scrollback_limit, expected.scrollback_limit);
        assert_eq!(actual.font_size, expected.font_size);
        assert_eq!(actual.cursor_style, expected.cursor_style);
        assert_eq!(actual.cursor_blink, expected.cursor_blink);
        assert_eq!(actual.screen_curvature, expected.screen_curvature);
        assert_eq!(actual.scanline_frequency, expected.scanline_frequency);
        assert_eq!(actual.glow_radius, expected.glow_radius);
        assert_eq!(actual.line_height, expected.line_height);
        assert_eq!(actual.profile_name, expected.profile_name);
        assert_eq!(actual.shader_preset, expected.shader_preset);
        assert_eq!(actual.prompt_preset, expected.prompt_preset);
        assert_eq!(actual.font_name, expected.font_name);
        assert_eq!(actual.shell_path, expected.shell_path);
        assert_eq!(actual.pythia_provider, expected.pythia_provider);
        assert_eq!(actual.hf_model, expected.hf_model);
        assert_eq!(actual.gemini_model, expected.gemini_model);
        assert_eq!(actual.ollama_url, expected.ollama_url);
        assert_eq!(actual.ollama_model, expected.ollama_model);
    }

    #[test]
    fn test_parse_hyprlang_blocks() {
        let input = r#"
        # Sample well.hl config
        appearance {
            theme_id = 2
            background_opacity = 0.88
            glass_blur_radius = 18.5
            font_size = 15.0
            line_height = 1.4
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
        assert_eq!(config.line_height, 1.4);
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
        let original = TheiaConfigPayload {
            theme_id: 7,
            background_opacity: 0.734_567,
            glass_blur_radius: 17.25,
            scan_timeout_ms: 41,
            command_timeout_ms: 913,
            enable_transient_prompt: false,
            enable_kitty_keyboard: false,
            scrollback_limit: 345_678,
            font_size: 16.75,
            cursor_style: 2,
            cursor_blink: false,
            screen_curvature: 0.137,
            scanline_frequency: 0.825,
            glow_radius: 2.375,
            line_height: 1.375,
            profile_name: Some("null".to_string()),
            shader_preset: 4,
            prompt_preset: "Night #1, \"focused\"\nmode".to_string(),
            font_name: "JetBrains Mono #2".to_string(),
            shell_path: "/opt/My Shell/bin/zsh\\preview".to_string(),
            pythia_provider: "Local, private #1".to_string(),
            hf_model: "org/model-with-\"quotes\"".to_string(),
            hf_token: "must-not-be-exported".to_string(),
            gemini_api_key: "also-must-not-be-exported".to_string(),
            gemini_model: "gemini/test,latest".to_string(),
            ollama_url: "http://localhost:11434/api#fragment".to_string(),
            ollama_model: "qwen:test,latest".to_string(),
        };

        let bindings = vec![
            KeybindingRow {
                chord: "SUPER+Return".to_string(),
                action: "exec: fish --command 'printf a,b # c'".to_string(),
                conflict: false,
            },
            KeybindingRow {
                chord: "CTRL+SHIFT+T".to_string(),
                action: "new_tab".to_string(),
                conflict: false,
            },
            KeybindingRow {
                chord: "F11".to_string(),
                action: "toggle: \"full, screen\"".to_string(),
                conflict: false,
            },
        ];

        let emitted = HyprlangEmitter::emit(&original, &bindings);
        assert!(!emitted.contains("hf_token"));
        assert!(!emitted.contains("gemini_api_key"));
        assert!(!emitted.contains("must-not-be-exported"));
        assert!(!emitted.contains("also-must-not-be-exported"));

        let parsed_doc = HyprlangParser::parse(&emitted).expect("Failed to parse emitted Hyprlang");
        let restored = HyprlangParser::to_theia_config(&parsed_doc);
        let restored_bindings = HyprlangParser::to_keybinding_rows(&parsed_doc);

        assert_non_secret_fields_eq(&restored, &original);
        assert!(restored.hf_token.is_empty());
        assert!(restored.gemini_api_key.is_empty());
        assert_eq!(restored_bindings.len(), bindings.len());
        for (actual, expected) in restored_bindings.iter().zip(&bindings) {
            assert_eq!(actual.chord, expected.chord);
            assert_eq!(actual.action, expected.action);
            assert!(!actual.conflict);
        }
    }

    #[test]
    fn absent_keys_preserve_defaults() {
        let document = HyprlangParser::parse(
            r#"
            appearance {
                theme_id = 9
            }
            "#,
        )
        .expect("partial Hyprlang should parse");
        let actual = HyprlangParser::to_theia_config(&document);
        let expected = TheiaConfigPayload {
            theme_id: 9,
            ..Default::default()
        };

        assert_non_secret_fields_eq(&actual, &expected);
    }

    #[test]
    fn top_level_legacy_assignments_remain_supported() {
        let document = HyprlangParser::parse(
            r#"
            font_name = Legacy Mono
            shell_path = /bin/fish
            enable_kitty_keyboard = false
            "#,
        )
        .expect("legacy top-level assignments should parse");
        let config = HyprlangParser::to_theia_config(&document);

        assert_eq!(config.font_name, "Legacy Mono");
        assert_eq!(config.shell_path, "/bin/fish");
        assert!(!config.enable_kitty_keyboard);
    }

    #[test]
    fn malformed_block_structure_is_rejected() {
        let unclosed = HyprlangParser::parse("appearance {\n theme_id = 1")
            .expect_err("unclosed blocks must fail");
        assert!(matches!(unclosed, HyprlangParseError::UnclosedBlock(_)));

        let nested = HyprlangParser::parse("appearance {\n nested {\n}\n}")
            .expect_err("nested blocks are unsupported");
        assert!(matches!(nested, HyprlangParseError::InvalidSyntax(2, _)));
    }
}
