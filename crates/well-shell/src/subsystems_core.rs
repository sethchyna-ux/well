//! Compatibility exports for the original subsystem API, backed by canonical implementations.
pub use crate::{MetisHistory, MetisTrieNode};
use std::io;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct TheiaConfigPayload {
    pub theme_id: u32,
    pub background_opacity: f32,
    pub glass_blur_radius: f32,
    pub scan_timeout_ms: u32,
    pub command_timeout_ms: u32,
    pub enable_transient_prompt: bool,
    pub enable_kitty_keyboard: bool,
    pub scrollback_limit: u32,
}
impl Default for TheiaConfigPayload {
    fn default() -> Self {
        Self {
            theme_id: 1,
            background_opacity: 1.0,
            glass_blur_radius: 0.0,
            scan_timeout_ms: 30,
            command_timeout_ms: 500,
            enable_transient_prompt: false,
            enable_kitty_keyboard: false,
            scrollback_limit: 10_000,
        }
    }
}
pub struct HermesChannel(well_ipc::HermesChannel<TheiaConfigPayload>);
impl HermesChannel {
    pub fn new(initial: TheiaConfigPayload) -> Self {
        Self(well_ipc::HermesChannel::new(initial))
    }
    pub fn sync_theia_state(&self, state: TheiaConfigPayload) {
        self.0.sync_state(state);
    }
    pub fn read_config_state(&self) -> TheiaConfigPayload {
        self.0.read_state()
    }
}

pub mod tree_sitter {
    pub use well_editor::{HighlightSpan, SyntaxClass as NodeKind};
}
pub struct MnemeTreeSitterMap {
    editor: well_editor::MnemeEditor,
    pub spans: Vec<well_editor::HighlightSpan>,
}
impl Default for MnemeTreeSitterMap {
    fn default() -> Self {
        Self::new()
    }
}
impl MnemeTreeSitterMap {
    pub fn new() -> Self {
        Self {
            editor: well_editor::MnemeEditor::default(),
            spans: Vec::new(),
        }
    }
    pub fn parse_incremental_rope(&mut self, rope: &ropey::Rope) -> io::Result<()> {
        self.editor.set_text(&rope.to_string());
        if self.editor.syntax_tree().is_none() {
            return Err(io::Error::other("Shell syntax parsing failed"));
        }
        self.spans = self.editor.highlights();
        Ok(())
    }
    pub fn render_ansi_highlighted(&self, rope: &ropey::Rope) -> String {
        use well_editor::SyntaxClass;
        let text = rope.to_string();
        if text != self.editor.get_text() {
            return text;
        }
        let mut output = String::new();
        let mut end = 0;
        for span in &self.spans {
            if span.start_byte < end {
                continue;
            }
            let (Some(prefix), Some(token)) = (
                text.get(end..span.start_byte),
                text.get(span.start_byte..span.end_byte),
            ) else {
                continue;
            };
            output.push_str(prefix);
            let color = match span.kind {
                SyntaxClass::Command => "32",
                SyntaxClass::Parameter => "36",
                SyntaxClass::Operator => "35",
                SyntaxClass::String => "33",
                SyntaxClass::Comment => "90",
                SyntaxClass::Variable => "34",
                SyntaxClass::Keyword => "35",
                SyntaxClass::InvalidSyntax => "1;31",
            };
            output.push_str(&format!("\x1b[{color}m{token}\x1b[0m"));
            end = span.end_byte;
        }
        output.push_str(&text[end..]);
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compatibility_highlighter_uses_the_real_grammar() {
        let mut map = MnemeTreeSitterMap::new();
        let rope = ropey::Rope::from_str("printf 'café' | cat # λ\n");
        map.parse_incremental_rope(&rope).unwrap();
        assert!(map
            .spans
            .iter()
            .any(|s| s.kind == well_editor::SyntaxClass::Comment));
        assert!(map.render_ansi_highlighted(&rope).contains("café"));
        assert_eq!(
            map.render_ansi_highlighted(&ropey::Rope::from_str("changed")),
            "changed"
        );
    }
}
