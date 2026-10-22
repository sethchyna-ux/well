//! Rope-backed composition editing with exact UTF-8 incremental syntax updates.
use ropey::Rope;
use std::io;
use std::ops::Range;
use tree_sitter::{InputEdit, Language, LanguageError, Node, Parser, Point, Tree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WellInputMode {
    CommandInput,
    Composition,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspDiagnostic {
    /// Character offsets, not bytes or UTF-16 positions.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxClass {
    Command,
    Parameter,
    Operator,
    String,
    Comment,
    Variable,
    Keyword,
    InvalidSyntax,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub kind: SyntaxClass,
}
#[derive(Clone)]
struct Snapshot {
    buffer: Rope,
    cursors: Vec<usize>,
    anchor: Option<usize>,
}

pub struct MnemeEditor {
    buffer: Rope,
    edit_mode: MnemeEditMode,
    cursors: Vec<usize>,
    selection_anchor: Option<usize>,
    ts_parser: Parser,
    ts_tree: Option<Tree>,
    diagnostics: Vec<LspDiagnostic>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    version: u64,
}
impl Default for MnemeEditor {
    fn default() -> Self {
        Self::new(None)
    }
}
impl MnemeEditor {
    /// Use the built-in Bash grammar unless a caller supplies a different grammar.
    pub fn new(language: Option<Language>) -> Self {
        Self::try_new(language)
            .expect("The built-in shell grammar must match the linked Tree-sitter ABI")
    }
    pub fn try_new(language: Option<Language>) -> Result<Self, LanguageError> {
        let mut parser = Parser::new();
        parser.set_language(&language.unwrap_or_else(|| tree_sitter_bash::LANGUAGE.into()))?;
        let mut editor = Self {
            buffer: Rope::new(),
            edit_mode: MnemeEditMode::Insert,
            cursors: vec![0],
            selection_anchor: None,
            ts_parser: parser,
            ts_tree: None,
            diagnostics: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            version: 0,
        };
        editor.reparse_buffer();
        Ok(editor)
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            buffer: self.buffer.clone(),
            cursors: self.cursors.clone(),
            anchor: self.selection_anchor,
        }
    }
    fn begin_edit(&mut self) {
        if self.undo.len() == 256 {
            self.undo.remove(0);
        }
        self.undo.push(self.snapshot());
        self.redo.clear();
    }
    fn finish_edit(&mut self) {
        self.version = self.version.wrapping_add(1);
        self.diagnostics.clear();
        self.reparse_buffer();
    }
    pub fn set_text(&mut self, text: &str) {
        if self.buffer == text {
            return;
        }
        let _ = self.replace_range(0..self.buffer.len_chars(), text);
    }
    pub fn set_cursors(&mut self, mut cursors: Vec<usize>) -> io::Result<()> {
        if cursors.is_empty() || cursors.iter().any(|&p| p > self.buffer.len_chars()) {
            return Err(invalid_range());
        }
        cursors.sort_unstable();
        cursors.dedup();
        self.cursors = cursors;
        self.selection_anchor = None;
        Ok(())
    }
    pub fn set_selection(&mut self, anchor: usize, cursor: usize) -> io::Result<()> {
        if anchor > self.buffer.len_chars() || cursor > self.buffer.len_chars() {
            return Err(invalid_range());
        }
        self.cursors = vec![cursor];
        self.selection_anchor = Some(anchor);
        Ok(())
    }
    pub fn selection(&self) -> Option<Range<usize>> {
        let anchor = self.selection_anchor?;
        let cursor = self.cursors[0];
        if anchor == cursor {
            None
        } else {
            Some(anchor.min(cursor)..anchor.max(cursor))
        }
    }
    pub fn selected_text(&self) -> Option<String> {
        self.selection().map(|r| self.buffer.slice(r).to_string())
    }
    pub fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.begin_edit();
        if let Some(range) = self.selection() {
            self.replace_raw(range, text);
        } else {
            let positions = self.cursors.clone();
            for position in positions.into_iter().rev() {
                self.replace_raw(position..position, text);
            }
        }
        self.selection_anchor = None;
        self.finish_edit();
    }
    pub fn replace_range(&mut self, range: Range<usize>, text: &str) -> io::Result<()> {
        if range.start > range.end || range.end > self.buffer.len_chars() {
            return Err(invalid_range());
        }
        if range.is_empty() && text.is_empty() {
            return Ok(());
        }
        self.begin_edit();
        self.replace_raw(range, text);
        self.selection_anchor = None;
        self.finish_edit();
        Ok(())
    }
    pub fn delete_range(&mut self, range: Range<usize>) {
        let _ = self.replace_range(range, "");
    }
    fn replace_raw(&mut self, range: Range<usize>, text: &str) {
        let start_byte = self.buffer.char_to_byte(range.start);
        let old_end_byte = self.buffer.char_to_byte(range.end);
        let start_point = self.point_at_byte(start_byte);
        let old_end_point = self.point_at_byte(old_end_byte);
        let removed_chars = range.end - range.start;
        let inserted_chars = text.chars().count();
        self.buffer.remove(range.clone());
        self.buffer.insert(range.start, text);
        let new_end_byte = start_byte + text.len();
        let new_end_point = self.point_at_byte(new_end_byte);
        if let Some(tree) = &mut self.ts_tree {
            tree.edit(&InputEdit {
                start_byte,
                old_end_byte,
                new_end_byte,
                start_position: start_point,
                old_end_position: old_end_point,
                new_end_position: new_end_point,
            });
        }
        let move_offset = |position: &mut usize| {
            if *position >= range.end {
                *position = *position - removed_chars + inserted_chars;
            } else if *position > range.start {
                *position = range.start + inserted_chars;
            }
        };
        for cursor in &mut self.cursors {
            move_offset(cursor);
        }
        if let Some(anchor) = &mut self.selection_anchor {
            move_offset(anchor);
        }
    }
    fn point_at_byte(&self, byte: usize) -> Point {
        // Ropey is configured for LF-only line breaks, matching Tree-sitter Points.
        let row = self.buffer.byte_to_line(byte);
        Point {
            row,
            column: byte - self.buffer.line_to_byte(row),
        }
    }
    fn reparse_buffer(&mut self) {
        let rope = &self.buffer;
        self.ts_tree = self.ts_parser.parse_with_options(
            &mut |byte, _| {
                if byte >= rope.len_bytes() {
                    return &[] as &[u8];
                }
                let (chunk, start, _, _) = rope.chunk_at_byte(byte);
                &chunk.as_bytes()[byte - start..]
            },
            self.ts_tree.as_ref(),
            None,
        );
    }
    fn restore(&mut self, state: Snapshot) {
        self.buffer = state.buffer;
        self.cursors = state.cursors;
        self.selection_anchor = state.anchor;
        self.ts_tree = None;
        self.finish_edit();
    }
    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.snapshot());
        self.restore(previous);
        true
    }
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.snapshot());
        self.restore(next);
        true
    }
    pub fn update_diagnostics(&mut self, diagnostics: Vec<LspDiagnostic>) {
        self.update_diagnostics_for_version(self.version, diagnostics);
    }
    /// Refuse stale language-server results after any edit/undo/redo.
    pub fn update_diagnostics_for_version(
        &mut self,
        version: u64,
        diagnostics: Vec<LspDiagnostic>,
    ) -> bool {
        if version != self.version {
            return false;
        }
        self.diagnostics = diagnostics
            .into_iter()
            .filter(|d| d.range.start <= d.range.end && d.range.end <= self.buffer.len_chars())
            .collect();
        true
    }
    pub fn diagnostics(&self) -> &[LspDiagnostic] {
        &self.diagnostics
    }
    pub fn syntax_tree(&self) -> Option<&Tree> {
        self.ts_tree.as_ref()
    }
    pub fn has_syntax_error(&self) -> bool {
        self.ts_tree
            .as_ref()
            .is_some_and(|t| t.root_node().has_error())
    }
    pub fn highlights(&self) -> Vec<HighlightSpan> {
        let mut spans = Vec::new();
        if let Some(tree) = &self.ts_tree {
            self.highlight_node(tree.root_node(), &mut spans);
        }
        spans
    }
    fn highlight_node(&self, node: Node<'_>, spans: &mut Vec<HighlightSpan>) {
        let kind = if node.is_error() || node.is_missing() {
            Some(SyntaxClass::InvalidSyntax)
        } else {
            match node.kind() {
                "command_name" => Some(SyntaxClass::Command),
                "string" | "raw_string" | "ansi_c_string" | "heredoc_body" => {
                    Some(SyntaxClass::String)
                }
                "comment" => Some(SyntaxClass::Comment),
                "variable_name" => Some(SyntaxClass::Variable),
                "if" | "then" | "else" | "elif" | "fi" | "for" | "do" | "done" | "while"
                | "until" | "case" | "esac" | "function" => Some(SyntaxClass::Keyword),
                "|" | "||" | "&&" | ">" | ">>" | "<" | ";" | "&" | "=" => {
                    Some(SyntaxClass::Operator)
                }
                "word" if self.buffer.byte_slice(node.byte_range()).chars().next() == Some('-') => {
                    Some(SyntaxClass::Parameter)
                }
                _ => None,
            }
        };
        if let Some(kind) = kind {
            if node.start_byte() < node.end_byte() {
                spans.push(HighlightSpan {
                    start_byte: node.start_byte(),
                    end_byte: node.end_byte(),
                    kind,
                });
            }
            return;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.highlight_node(child, spans);
        }
    }
    pub fn version(&self) -> u64 {
        self.version
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
fn invalid_range() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "Editor offsets must be within the character buffer",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn assert_matches_fresh(editor: &MnemeEditor) {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_bash::LANGUAGE.into())
            .unwrap();
        let fresh = parser.parse(editor.get_text(), None).unwrap();
        let current = editor.syntax_tree().unwrap();
        assert_eq!(current.root_node().to_sexp(), fresh.root_node().to_sexp());
        assert_eq!(current.root_node().end_byte(), fresh.root_node().end_byte());
        assert_eq!(
            current.root_node().end_position(),
            fresh.root_node().end_position()
        );
    }
    #[test]
    fn utf8_multiline_edits_match_a_fresh_parse() {
        let mut editor = MnemeEditor::default();
        editor.insert_text("printf 'café 🦀'\n# λ\nprintf x\n");
        assert_matches_fresh(&editor);
        for (start, end, text) in [
            (8, 12, "e\u{301}界"),
            (0, 0, "# 😀\n"),
            (3, 7, "\r"),
            (0, 2, "\u{2028}"),
            (1, 4, ""),
        ] {
            editor.replace_range(start..end, text).unwrap();
            assert_matches_fresh(&editor);
        }
        editor.replace_range(0..editor.len_chars(), "").unwrap();
        assert_matches_fresh(&editor);
    }
    #[test]
    fn selection_multicursor_undo_and_redo_preserve_offsets() {
        let mut editor = MnemeEditor::default();
        editor.insert_text("écho λ");
        editor.set_selection(0, 4).unwrap();
        assert_eq!(editor.selected_text().as_deref(), Some("écho"));
        editor.insert_text("printf");
        assert_eq!(editor.get_text(), "printf λ");
        assert_eq!(editor.cursors(), &[6]);
        assert!(editor.undo());
        assert_eq!(editor.get_text(), "écho λ");
        assert!(editor.redo());
        assert_eq!(editor.get_text(), "printf λ");
        editor.set_cursors(vec![0, editor.len_chars()]).unwrap();
        editor.insert_text("!");
        assert_eq!(editor.get_text(), "!printf λ!");
        assert_eq!(editor.cursors(), &[1, 10]);
        assert!(editor.undo());
        assert_eq!(editor.get_text(), "printf λ");
        assert_matches_fresh(&editor);
        editor.insert_text("x");
        assert!(!editor.redo());
    }
    #[test]
    fn diagnostics_are_versioned_and_bad_ranges_are_rejected() {
        let mut editor = MnemeEditor::default();
        let version = editor.version();
        editor.insert_text("printf x");
        let diagnostic = LspDiagnostic {
            range: 0..2,
            severity: DiagnosticSeverity::Warning,
            message: "test diagnostic".into(),
        };
        assert!(!editor.update_diagnostics_for_version(version, vec![diagnostic.clone()]));
        assert!(editor.update_diagnostics_for_version(editor.version(), vec![diagnostic]));
        assert_eq!(editor.diagnostics().len(), 1);
        editor.insert_text("y");
        assert!(editor.diagnostics().is_empty());
        let text = editor.get_text();
        assert!(editor.replace_range(100..101, "x").is_err());
        assert_eq!(text, editor.get_text());
        assert!(editor.set_cursors(vec![100]).is_err());
    }
    #[test]
    fn syntax_and_highlights_come_from_the_grammar() {
        let mut editor = MnemeEditor::default();
        editor.set_text("printf --help | cat # real comment\n");
        assert!(!editor.has_syntax_error());
        let spans = editor.highlights();
        assert!(spans.iter().any(|s| s.kind == SyntaxClass::Command));
        assert!(spans.iter().any(|s| s.kind == SyntaxClass::Comment));
        assert!(spans
            .windows(2)
            .all(|pair| pair[0].end_byte <= pair[1].start_byte));
        editor.set_text("if true; then\n");
        assert!(editor.has_syntax_error());
        assert_matches_fresh(&editor);
    }
}
