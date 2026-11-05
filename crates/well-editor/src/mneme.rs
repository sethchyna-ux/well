//! Rope-backed composition editing with exact UTF-8 incremental syntax updates
//! and background Safety AST validation.

use crate::snippets::{ExpandedTabStop, WellSnippet};
use crate::validator::SafetyAstValidator;
use ropey::Rope;
use serde::{Deserialize, Serialize};
use std::io;
use std::ops::Range;
use tree_sitter::{InputEdit, Language, LanguageError, Node, Parser, Point, Tree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WellInputMode {
    CommandInput,
    Composition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspDiagnostic {
    /// Character offsets, not bytes or UTF-16 positions.
    pub range: Range<usize>,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    Builtin,
    Parameter,
    Operator,
    Delimiter,
    String,
    Number,
    Comment,
    Variable,
    Keyword,
    InvalidSyntax,
    ValidPath,
    InvalidPath,
    UnclosedDelimiter,
    Destructive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorMovement {
    Left,
    Right,
    Up,
    Down,
    LineStart,
    LineEnd,
    WordForward,
    WordBackward,
    BufferStart,
    BufferEnd,
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
    validator: SafetyAstValidator,
    cached_highlights: Vec<HighlightSpan>,
    is_unclosed: bool,
    unclosed_reason: Option<String>,
    destructive_warning: Option<String>,
    snippet_stops: Vec<ExpandedTabStop>,
    active_stop_idx: Option<usize>,
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
            validator: SafetyAstValidator::new(),
            cached_highlights: Vec::new(),
            is_unclosed: false,
            unclosed_reason: None,
            destructive_warning: None,
            snippet_stops: Vec::new(),
            active_stop_idx: None,
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
        self.destructive_warning = None;
        self.reparse_buffer();
        // Submit to background Safety AST validator
        self.validator.submit(self.version, self.buffer.to_string());
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

    /// Smart auto-indentation: inserts newline and calculates appropriate indent level.
    pub fn insert_newline_with_indent(&mut self) {
        let cursor = self.cursors[0];
        let line_idx = self.buffer.char_to_line(cursor);
        let line = self.buffer.line(line_idx).to_string();

        let mut indent_spaces = 0;
        for ch in line.chars() {
            if ch == ' ' {
                indent_spaces += 1;
            } else if ch == '\t' {
                indent_spaces += 4;
            } else {
                break;
            }
        }

        let trimmed = line.trim();
        // If line ends with opening block statement, indent by 2 extra spaces
        if trimmed.ends_with("do")
            || trimmed.ends_with("then")
            || trimmed.ends_with('{')
            || trimmed.ends_with('(')
            || trimmed.ends_with('\\')
        {
            indent_spaces += 2;
        }

        let mut insertion = String::with_capacity(indent_spaces + 1);
        insertion.push('\n');
        for _ in 0..indent_spaces {
            insertion.push(' ');
        }

        self.insert_text(&insertion);
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

    pub fn delete_line(&mut self) {
        let cursor = self.cursors[0];
        let line_idx = self.buffer.char_to_line(cursor);
        let start = self.buffer.line_to_char(line_idx);
        let next_line = line_idx + 1;
        let end = if next_line < self.buffer.len_lines() {
            self.buffer.line_to_char(next_line)
        } else {
            self.buffer.len_chars()
        };
        self.delete_range(start..end);
    }

    pub fn backspace(&mut self) {
        if let Some(cursor) = self.cursors.first().copied() {
            if cursor > 0 {
                self.delete_range(cursor - 1..cursor);
            }
        }
    }

    pub fn delete_next_char(&mut self) {
        if let Some(cursor) = self.cursors.first().copied() {
            if cursor < self.buffer.len_chars() {
                self.delete_range(cursor..cursor + 1);
            }
        }
    }

    pub fn take_execution_payload(&mut self) -> Option<String> {
        let text = self.get_text();
        self.snippet_stops.clear();
        self.active_stop_idx = None;
        self.set_text("");
        if text.trim().is_empty() {
            None
        } else {
            Some(text)
        }
    }

    /// Expand a `WellSnippet` at `replace_range` (or current cursor if `None`) and select the first tab-stop.
    pub fn expand_snippet(&mut self, snippet: &WellSnippet, replace_range: Option<Range<usize>>) {
        let target_range = replace_range.unwrap_or_else(|| {
            let c = self.cursors.first().copied().unwrap_or(0);
            c..c
        });
        let base_offset = target_range.start;
        let expanded = snippet.expand();
        let _ = self.replace_range(target_range, &expanded.text);

        let shifted_stops: Vec<ExpandedTabStop> = expanded
            .tab_stops
            .into_iter()
            .map(|mut stop| {
                stop.char_range =
                    (base_offset + stop.char_range.start)..(base_offset + stop.char_range.end);
                stop
            })
            .collect();

        if shifted_stops.is_empty() {
            self.snippet_stops.clear();
            self.active_stop_idx = None;
        } else {
            let first = shifted_stops[0].char_range.clone();
            let _ = self.set_selection(first.start, first.end);
            self.snippet_stops = shifted_stops;
            self.active_stop_idx = Some(0);
        }
    }

    /// Advance to the next snippet tab-stop if an expanded snippet is active.
    /// Returns `true` if a tab-stop jump occurred.
    pub fn next_snippet_placeholder(&mut self) -> bool {
        let Some(curr_idx) = self.active_stop_idx else {
            return false;
        };
        let next_idx = curr_idx + 1;
        if next_idx < self.snippet_stops.len() {
            let range = self.snippet_stops[next_idx].char_range.clone();
            let max_len = self.buffer.len_chars();
            let start = range.start.min(max_len);
            let end = range.end.min(max_len);
            let _ = self.set_selection(start, end);
            self.active_stop_idx = Some(next_idx);
            true
        } else {
            // Final stop completed; move cursor to end of last stop and clear active stops
            if let Some(last) = self.snippet_stops.last() {
                let end = last.char_range.end.min(self.buffer.len_chars());
                let _ = self.set_cursors(vec![end]);
            }
            self.snippet_stops.clear();
            self.active_stop_idx = None;
            true
        }
    }

    /// Check if the token immediately preceding the primary cursor matches a snippet trigger in `catalog`,
    /// or if a snippet is already active, advance to the next placeholder.
    pub fn expand_or_advance_snippet(&mut self, catalog: &[WellSnippet], is_ssh: bool) -> bool {
        let cursor = self.cursors.first().copied().unwrap_or(0);
        let prefix = self.buffer.slice(0..cursor).to_string();
        let token_start_char = prefix
            .chars()
            .rev()
            .take_while(|c| !c.is_whitespace())
            .count();
        if token_start_char > 0 {
            let start = cursor - token_start_char;
            let token = self.buffer.slice(start..cursor).to_string();
            if let Some(snippet) = catalog
                .iter()
                .find(|s| s.trigger == token && s.scope.matches_context(is_ssh))
            {
                let snippet_clone = snippet.clone();
                self.expand_snippet(&snippet_clone, Some(start..cursor));
                return true;
            }
        }
        self.next_snippet_placeholder()
    }

    pub fn active_snippet_stop(&self) -> Option<(usize, usize, Range<usize>)> {
        let idx = self.active_stop_idx?;
        let stop = self.snippet_stops.get(idx)?;
        Some((idx + 1, self.snippet_stops.len(), stop.char_range.clone()))
    }

    pub fn move_cursor_up(&mut self) {
        self.move_cursor(CursorMovement::Up, false);
    }

    pub fn move_cursor_down(&mut self) {
        self.move_cursor(CursorMovement::Down, false);
    }

    pub fn move_cursor_left(&mut self) {
        self.move_cursor(CursorMovement::Left, false);
    }

    pub fn move_cursor_right(&mut self) {
        self.move_cursor(CursorMovement::Right, false);
    }

    pub fn move_word_left(&mut self) {
        self.move_cursor(CursorMovement::WordBackward, false);
    }

    pub fn move_word_right(&mut self) {
        self.move_cursor(CursorMovement::WordForward, false);
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
        for stop in &mut self.snippet_stops {
            let mut s = stop.char_range.start;
            let mut e = stop.char_range.end;
            if s > range.start {
                move_offset(&mut s);
            }
            move_offset(&mut e);
            stop.char_range = s..e.max(s);
        }
    }

    fn point_at_byte(&self, byte: usize) -> Point {
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

    /// Poll background Safety AST validator results.
    pub fn poll_validator(&mut self) {
        if let Some(res) = self.validator.poll_result() {
            if res.version == self.version {
                self.cached_highlights = res.spans.clone();
                self.is_unclosed = res.is_unclosed;
                self.unclosed_reason = res.unclosed_construct.clone();
                self.destructive_warning = res.destructive_warning.clone();
                if !res.diagnostics.is_empty() {
                    self.diagnostics = res.diagnostics.clone();
                }
            }
        }
    }

    pub fn is_unclosed_block(&mut self) -> bool {
        self.poll_validator();
        self.is_unclosed
    }

    pub fn unclosed_reason(&mut self) -> Option<String> {
        self.poll_validator();
        self.unclosed_reason.clone()
    }

    pub fn destructive_warning(&mut self) -> Option<String> {
        self.poll_validator();
        self.destructive_warning.clone()
    }

    /// Synchronous unclosed check for prompt expansion in the main terminal loop.
    pub fn is_command_unclosed(cmd: &str) -> (bool, Option<String>) {
        SafetyAstValidator::check_unclosed_sync(cmd)
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

    /// Highlighting spans with path validity and unclosed delimiter classification.
    pub fn highlights(&mut self) -> Vec<HighlightSpan> {
        self.poll_validator();
        if !self.cached_highlights.is_empty() {
            return self.cached_highlights.clone();
        }
        // Fallback synchronous walk if background result hasn't arrived yet
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
                | "until" | "case" | "esac" | "function" | "in" => Some(SyntaxClass::Keyword),
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

    /// Move the primary cursor using semantic movement.
    pub fn move_cursor(&mut self, movement: CursorMovement, extend_selection: bool) {
        let cursor = self.cursors[0];
        let total_chars = self.buffer.len_chars();

        let new_cursor = match movement {
            CursorMovement::Left => cursor.saturating_sub(1),
            CursorMovement::Right => (cursor + 1).min(total_chars),
            CursorMovement::Up => {
                let line_idx = self.buffer.char_to_line(cursor);
                if line_idx > 0 {
                    let col = cursor - self.buffer.line_to_char(line_idx);
                    let prev_line = line_idx - 1;
                    let prev_line_start = self.buffer.line_to_char(prev_line);
                    let prev_len = self.buffer.line(prev_line).len_chars().saturating_sub(1);
                    prev_line_start + col.min(prev_len)
                } else {
                    0
                }
            }
            CursorMovement::Down => {
                let line_idx = self.buffer.char_to_line(cursor);
                if line_idx + 1 < self.buffer.len_lines() {
                    let col = cursor - self.buffer.line_to_char(line_idx);
                    let next_line = line_idx + 1;
                    let next_line_start = self.buffer.line_to_char(next_line);
                    let next_len = self.buffer.line(next_line).len_chars().saturating_sub(1);
                    (next_line_start + col.min(next_len)).min(total_chars)
                } else {
                    total_chars
                }
            }
            CursorMovement::LineStart => {
                let line_idx = self.buffer.char_to_line(cursor);
                self.buffer.line_to_char(line_idx)
            }
            CursorMovement::LineEnd => {
                let line_idx = self.buffer.char_to_line(cursor);
                let line_len = self.buffer.line(line_idx).len_chars();
                let start = self.buffer.line_to_char(line_idx);
                (start + line_len.saturating_sub(1)).min(total_chars)
            }
            CursorMovement::WordForward => {
                let mut pos = cursor;
                let text = self.buffer.slice(pos..total_chars).to_string();
                let mut chars = text.chars().peekable();
                // Skip non-whitespace then whitespace
                while let Some(ch) = chars.next() {
                    pos += 1;
                    if ch.is_whitespace() {
                        while let Some(&next_ch) = chars.peek() {
                            if next_ch.is_whitespace() {
                                chars.next();
                                pos += 1;
                            } else {
                                break;
                            }
                        }
                        break;
                    }
                }
                pos.min(total_chars)
            }
            CursorMovement::WordBackward => {
                let mut pos = cursor;
                let text = self.buffer.slice(0..pos).to_string();
                let mut chars = text.chars().rev().peekable();
                while let Some(ch) = chars.next() {
                    pos = pos.saturating_sub(1);
                    if !ch.is_whitespace() {
                        while let Some(&next_ch) = chars.peek() {
                            if !next_ch.is_whitespace() {
                                chars.next();
                                pos = pos.saturating_sub(1);
                            } else {
                                break;
                            }
                        }
                        break;
                    }
                }
                pos
            }
            CursorMovement::BufferStart => 0,
            CursorMovement::BufferEnd => total_chars,
        };

        if extend_selection {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(cursor);
            }
        } else {
            self.selection_anchor = None;
        }
        self.cursors = vec![new_cursor];
    }

    pub fn cursor_line(&self) -> usize {
        self.buffer.char_to_line(self.cursors[0])
    }

    pub fn cursor_column(&self) -> usize {
        let cursor = self.cursors[0];
        let line_idx = self.buffer.char_to_line(cursor);
        cursor - self.buffer.line_to_char(line_idx)
    }

    pub fn len_lines(&self) -> usize {
        self.buffer.len_lines()
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

impl std::fmt::Display for MnemeEditor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.buffer)
    }
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

    #[test]
    fn test_unclosed_block_detection() {
        let (unclosed, reason) = MnemeEditor::is_command_unclosed("for f in *.txt; do\n");
        assert!(unclosed);
        assert!(reason.is_some());

        let (unclosed_if, reason_if) = MnemeEditor::is_command_unclosed("if [ -f foo ]; then\n");
        assert!(unclosed_if);
        assert!(reason_if.is_some());

        let (closed, _) = MnemeEditor::is_command_unclosed("for f in *.txt; do echo $f; done");
        assert!(!closed);
    }

    fn poll_warning(editor: &mut MnemeEditor) -> Option<String> {
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_millis(15));
            if let Some(w) = editor.destructive_warning() {
                return Some(w);
            }
        }
        None
    }

    #[test]
    fn test_destructive_command_safety_gate() {
        let mut editor = MnemeEditor::default();
        editor.set_text("rm -rf /\n");
        assert!(
            poll_warning(&mut editor).is_some(),
            "Destructive rm -rf / must be flagged"
        );

        // Test kubectl delete --all
        editor.set_text("kubectl delete deployments --all\n");
        assert!(
            poll_warning(&mut editor).is_some(),
            "kubectl delete --all must be flagged"
        );

        // Test git push --force main
        editor.set_text("git push origin main --force\n");
        assert!(
            poll_warning(&mut editor).is_some(),
            "git push force to main must be flagged"
        );

        // Test terraform destroy --auto-approve
        editor.set_text("terraform destroy --auto-approve\n");
        assert!(
            poll_warning(&mut editor).is_some(),
            "terraform destroy --auto-approve must be flagged"
        );
    }

    #[test]
    fn test_smart_indentation() {
        let mut editor = MnemeEditor::default();
        editor.set_text("for i in 1 2 3; do");
        editor.move_cursor(CursorMovement::BufferEnd, false);
        editor.insert_newline_with_indent();
        assert_eq!(editor.get_text(), "for i in 1 2 3; do\n  ");
    }

    #[test]
    fn test_cursor_movements() {
        let mut editor = MnemeEditor::default();
        editor.set_text("hello world\nline 2");
        editor.move_cursor(CursorMovement::BufferStart, false);
        assert_eq!(editor.cursors(), &[0]);
        editor.move_cursor(CursorMovement::WordForward, false);
        assert_eq!(editor.cursors(), &[6]); // Start of "world"
        editor.move_cursor(CursorMovement::LineEnd, false);
        assert_eq!(editor.cursors(), &[11]);
        editor.move_cursor(CursorMovement::Down, false);
        assert_eq!(editor.cursor_line(), 1);
    }

    #[test]
    fn test_parameterized_snippet_tab_stops_in_mneme() {
        let mut editor = MnemeEditor::default();
        let catalog = WellSnippet::builtin_catalog();
        editor.insert_text(":klogs");
        assert!(editor.expand_or_advance_snippet(&catalog, false));
        assert_eq!(editor.get_text(), "kubectl logs -f -n default pod-name");
        assert_eq!(editor.selected_text().as_deref(), Some("default"));

        // Replace first placeholder and press Tab to jump to second placeholder
        editor.insert_text("kube-system");
        assert_eq!(editor.get_text(), "kubectl logs -f -n kube-system pod-name");
        assert!(editor.expand_or_advance_snippet(&catalog, false));
        assert_eq!(editor.selected_text().as_deref(), Some("pod-name"));

        // Replace second placeholder and complete tab stops
        editor.insert_text("api-server-0");
        assert_eq!(
            editor.get_text(),
            "kubectl logs -f -n kube-system api-server-0"
        );
        assert!(editor.expand_or_advance_snippet(&catalog, false));
        assert!(editor.active_snippet_stop().is_none());
    }
}
