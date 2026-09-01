//! well-editor: Mneme Inline Rope Text Editor & AST Validator
//!
//! Subsystems:
//! - MnemeEditor: B-tree rope buffer (Ropey) providing O(log n) modifications.
//! - Tree-sitter incremental parsing & LSP diagnostic tracking.

use std::ops::Range;
use ropey::Rope;
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
        self.buffer.insert(insert_pos, text);
        let inserted_chars = text.chars().count();

        let start_point = self.offset_to_point(insert_pos);
        let end_point = self.offset_to_point(insert_pos + inserted_chars);

        if let Some(ref mut tree) = self.ts_tree {
            let edit = InputEdit {
                start_byte: insert_pos * 4,
                old_end_byte: insert_pos * 4,
                new_end_byte: (insert_pos + inserted_chars) * 4,
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

        let start_point = self.offset_to_point(range.start);
        let end_point = self.offset_to_point(range.end);
        let deleted_chars = range.end - range.start;

        self.buffer.remove(range.clone());

        if let Some(ref mut tree) = self.ts_tree {
            let edit = InputEdit {
                start_byte: range.start * 4,
                old_end_byte: range.end * 4,
                new_end_byte: range.start * 4,
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
            let char_idx = byte_idx / 4;
            if char_idx >= rope_ref.len_chars() {
                return &[];
            }
            let (slice, _, _, _) = rope_ref.chunk_at_char(char_idx);
            slice.as_bytes()
        };

        self.ts_tree = self.ts_parser.parse_with(&mut callback, self.ts_tree.as_ref());
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
            let last_line_chars = self.buffer.line(last_line).len_chars();
            return Point { row: last_line, column: last_line_chars };
        }
        let line_idx = self.buffer.char_to_line(offset);
        let line_start_char = self.buffer.line_to_char(line_idx);
        let column = offset - line_start_char;
        Point { row: line_idx, column }
    }

    pub fn get_text(&self) -> String {
        self.buffer.to_string()
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

    pub fn set_edit_mode(&mut self, mode: MnemeEditMode) {
        self.edit_mode = mode;
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
}
