//! well-editor: Mneme Inline Rope Text Editor & AST Validator
//!
//! Subsystems:
//! - MnemeEditor: B-tree rope buffer (Ropey) providing O(log n) modifications.
//! - Tree-sitter incremental parsing & LSP diagnostic tracking.

use ropey::Rope;
use std::ops::Range;
use tree_sitter::{InputEdit, Parser, Point, Tree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WellInputMode {
    CommandInput,
    Composition,
}

#[derive(Debug, Clone)]
pub struct LspDiagnostic {
    pub range: Range<usize>,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MnemeEditMode {
    Normal,
    Insert,
    VisualChar,
    VisualBlock,
}

pub struct MnemeEditor {
    buffer: Rope,
    edit_mode: MnemeEditMode,
    cursors: Vec<usize>,
    #[allow(dead_code)]
    selection_anchor: Option<usize>,
    ts_parser: Parser,
    ts_tree: Option<Tree>,
    diagnostics: Vec<LspDiagnostic>,
}

impl Default for MnemeEditor {
    fn default() -> Self {
        Self::new(None)
    }
}

impl MnemeEditor {
    pub fn new(lang: Option<tree_sitter::Language>) -> Self {
        let mut ts_parser = Parser::new();
        if let Some(language) = lang {
            let _ = ts_parser.set_language(&language);
        }

        Self {
            buffer: Rope::new(),
            edit_mode: MnemeEditMode::Insert,
            cursors: vec![0],
            selection_anchor: None,
            ts_parser,
            ts_tree: None,
            diagnostics: Vec::new(),
        }
    }

    pub fn insert_text(&mut self, text: &str) {
        if self.cursors.is_empty() {
            return;
        }

        let insert_pos = self.cursors[0];
        let start_byte = self.char_to_byte(insert_pos);
        self.buffer.insert(insert_pos, text);
        let inserted_chars = text.chars().count();
        let new_end_byte = self.char_to_byte(insert_pos + inserted_chars);

        let start_point = self.offset_to_point(insert_pos);
        let end_point = self.offset_to_point(insert_pos + inserted_chars);

        if let Some(ref mut tree) = self.ts_tree {
            let edit = InputEdit {
                start_byte,
                old_end_byte: start_byte,
                new_end_byte,
                start_position: start_point,
                old_end_position: start_point,
                new_end_position: end_point,
            };
            tree.edit(&edit);
        }

        for cursor in &mut self.cursors {
            if *cursor >= insert_pos {
                *cursor += inserted_chars;
            }
        }

        self.reparse_buffer();
    }

    pub fn delete_range(&mut self, range: Range<usize>) {
        if range.start >= range.end || range.end > self.buffer.len_chars() {
            return;
        }

        let start_byte = self.char_to_byte(range.start);
        let old_end_byte = self.char_to_byte(range.end);
        let start_point = self.offset_to_point(range.start);
        let end_point = self.offset_to_point(range.end);
        let deleted_chars = range.end - range.start;

        self.buffer.remove(range.clone());

        if let Some(ref mut tree) = self.ts_tree {
            let edit = InputEdit {
                start_byte,
                old_end_byte,
                new_end_byte: start_byte,
                start_position: start_point,
                old_end_position: end_point,
                new_end_position: start_point,
            };
            tree.edit(&edit);
        }

        for cursor in &mut self.cursors {
            if *cursor >= range.end {
                *cursor -= deleted_chars;
            } else if *cursor > range.start {
                *cursor = range.start;
            }
        }

        self.reparse_buffer();
    }

    fn reparse_buffer(&mut self) {
        let rope_ref = &self.buffer;
        let mut callback = |byte_idx: usize, _point: Point| -> &[u8] {
            let char_idx = rope_ref.byte_to_char(byte_idx.min(rope_ref.len_bytes()));
            if char_idx >= rope_ref.len_chars() {
                return &[];
            }
            let (slice, _, _, _) = rope_ref.chunk_at_char(char_idx);
            slice.as_bytes()
        };

        self.ts_tree = self
            .ts_parser
            .parse_with(&mut callback, self.ts_tree.as_ref());
    }

    pub fn update_diagnostics(&mut self, diagnostics: Vec<LspDiagnostic>) {
        self.diagnostics = diagnostics;
    }

    fn offset_to_point(&self, offset: usize) -> Point {
        if self.buffer.len_chars() == 0 {
            return Point { row: 0, column: 0 };
        }
        if offset >= self.buffer.len_chars() {
            let last_line = self.buffer.len_lines() - 1;
            let last_line_bytes = self.buffer.line(last_line).len_bytes();
            return Point {
                row: last_line,
                column: last_line_bytes,
            };
        }
        let line_idx = self.buffer.char_to_line(offset);
        let line_start_char = self.buffer.line_to_char(line_idx);
        let line_start_byte = self.buffer.char_to_byte(line_start_char);
        let offset_byte = self.buffer.char_to_byte(offset);
        let column = offset_byte - line_start_byte;
        Point {
            row: line_idx,
            column,
        }
    }

    pub fn get_text(&self) -> String {
        self.buffer.to_string()
    }

    pub fn set_text(&mut self, text: &str) {
        self.buffer = Rope::from_str(text);
        self.cursors = vec![self.buffer.len_chars()];
        self.reparse_buffer();
    }

    pub fn is_blank(&self) -> bool {
        self.buffer.to_string().trim().is_empty()
    }

    pub fn execution_payload(&self) -> Option<String> {
        let mut payload = self.get_text();
        if payload.trim().is_empty() {
            return None;
        }
        if !payload.ends_with('\r') && !payload.ends_with('\n') {
            payload.push('\r');
        }
        Some(payload)
    }

    pub fn take_execution_payload(&mut self) -> Option<String> {
        let payload = self.execution_payload();
        if payload.is_some() {
            self.clear();
        }
        payload
    }

    pub fn len_chars(&self) -> usize {
        self.buffer.len_chars()
    }

    pub fn cursors(&self) -> &[usize] {
        &self.cursors
    }

    pub fn edit_mode(&self) -> MnemeEditMode {
        self.edit_mode
    }

    pub fn clear(&mut self) {
        let len = self.buffer.len_chars();
        if len > 0 {
            self.delete_range(0..len);
        }
        self.cursors = vec![0];
    }

    pub fn backspace(&mut self) {
        if self.cursors.is_empty() {
            return;
        }
        let pos = self.cursors[0];
        if pos > 0 {
            self.delete_range((pos - 1)..pos);
        }
    }

    pub fn move_cursor_left(&mut self) {
        if !self.cursors.is_empty() && self.cursors[0] > 0 {
            self.cursors[0] -= 1;
        }
    }

    pub fn move_cursor_right(&mut self) {
        if !self.cursors.is_empty() && self.cursors[0] < self.buffer.len_chars() {
            self.cursors[0] += 1;
        }
    }

    pub fn set_edit_mode(&mut self, mode: MnemeEditMode) {
        self.edit_mode = mode;
    }

    fn char_to_byte(&self, char_idx: usize) -> usize {
        self.buffer
            .char_to_byte(char_idx.min(self.buffer.len_chars()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mneme_rope_edits() {
        let mut editor = MnemeEditor::default();
        editor.insert_text("fn main() {\n    println!(\"Well\");\n}\n");
        assert!(editor.get_text().contains("println!(\"Well\")"));
        assert_eq!(editor.len_chars(), 36);

        editor.delete_range(0..3);
        assert!(editor.get_text().starts_with("main()"));
    }

    #[test]
    fn set_text_replaces_composition_and_moves_cursor_to_end() {
        let mut editor = MnemeEditor::default();
        editor.insert_text("old");

        editor.set_text("echo Well\nprintf '✓'\n");

        assert_eq!(editor.get_text(), "echo Well\nprintf '✓'\n");
        assert_eq!(editor.cursors(), &[21]);
    }

    #[test]
    fn execution_payload_rejects_blank_compositions() {
        let mut editor = MnemeEditor::default();
        editor.set_text(" \n\t ");

        assert_eq!(editor.execution_payload(), None);
        assert!(editor.is_blank());
    }

    #[test]
    fn execution_payload_appends_carriage_return_when_needed() {
        let mut editor = MnemeEditor::default();
        editor.set_text("echo Well");

        assert_eq!(editor.execution_payload().as_deref(), Some("echo Well\r"));
    }

    #[test]
    fn execution_payload_preserves_existing_line_terminator() {
        let mut editor = MnemeEditor::default();
        editor.set_text("echo Well\n");

        assert_eq!(editor.execution_payload().as_deref(), Some("echo Well\n"));
    }

    #[test]
    fn taking_execution_payload_clears_only_executable_compositions() {
        let mut editor = MnemeEditor::default();
        editor.set_text("echo Well");

        assert_eq!(
            editor.take_execution_payload().as_deref(),
            Some("echo Well\r")
        );
        assert_eq!(editor.get_text(), "");

        editor.set_text("   ");
        assert_eq!(editor.take_execution_payload(), None);
        assert_eq!(editor.get_text(), "   ");
    }

    #[test]
    fn unicode_edits_keep_character_cursor_semantics() {
        let mut editor = MnemeEditor::default();
        editor.set_text("echo ✓");

        editor.move_cursor_left();
        editor.backspace();
        editor.insert_text("λ");

        assert_eq!(editor.get_text(), "echoλ✓");
        assert_eq!(editor.len_chars(), 6);
        assert_eq!(editor.cursors(), &[5]);
    }
}
