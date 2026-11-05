//! Mneme Editor Core (crates/well-editor/src/mneme.rs)
//!
//! Named after Mneme, the ancient Greek Muse of Memory, this module implements
//! the inline multi-line rope editor for the Well unified terminal system.
//! It handles in-buffer text compositions, AST syntax tree updates via Tree-sitter,
//! and background Language Server Protocol (LSP) diagnostics folding.

use std::ops::Range;
use std::sync::{Arc, RwLock};
use ropey::Rope;
use tree_sitter::{Parser, Tree, InputEdit, Point};

/// Architectural Modes for the Well unified input loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WellInputMode {
    /// metis: Interactive command-line shell prompt mode (single-line or basic multi-line)
    CommandInput,
    /// mneme: Dedicated visual inline multi-line rope text composition mode
    Composition,
}

/// Represents a single diagnostic lint received from an active LSP host.
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

/// The core visual state of the Mneme text composition frame.
pub struct MnemeEditor {
    /// The B-tree rope buffer representing the file contents in-memory.
    /// Btree ropes guarantee O(log n) text modifications for multi-megabyte buffers.
    buffer: Rope,
    /// Current editing and selection mode (Vim Normal, Insert, Visual).
    edit_mode: MnemeEditMode,
    /// Active cursor positions supporting multi-cursor operations.
    cursors: Vec<usize>,
    /// The primary visual selection anchor.
    selection_anchor: Option<usize>,
    /// Incremental Tree-sitter parser instance for code block AST generation.
    ts_parser: Parser,
    /// The compiled syntax tree representing the current buffer structure.
    ts_tree: Option<Tree>,
    /// Live diagnostics pulled asynchronously from background LSP workers.
    diagnostics: Vec<LspDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MnemeEditMode {
    Normal,
    Insert,
    VisualChar,
    VisualBlock,
}

impl MnemeEditor {
    /// Initialize a new Mneme editor frame with an empty B-tree rope buffer
    /// and a pre-configured Tree-sitter language parser.
    pub fn new(lang: Option<tree_sitter::Language>) -> Self {
        let mut ts_parser = Parser::new();
        if let Some(language) = lang {
            ts_parser.set_language(&language).expect("Failed to load Tree-sitter grammar for Mneme");
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

    /// Insert text at the primary cursor position using O(log n) rope operations,
    /// updating both our cursor offsets and parsing an incremental AST delta.
    pub fn insert_text(&mut self, text: &str) {
        if self.cursors.is_empty() {
            return;
        }

        let insert_pos = self.cursors[0];
        let old_char_count = self.buffer.len_chars();
        
        // Perform O(log n) B-Tree insertion
        self.buffer.insert(insert_pos, text);
        let inserted_chars = text.chars().count();

        // Calculate AST update metrics for incremental Tree-sitter parses
        let start_point = self.offset_to_point(insert_pos);
        let end_point = self.offset_to_point(insert_pos + inserted_chars);

        if let Some(ref mut tree) = self.ts_tree {
            let edit = InputEdit {
                start_byte: insert_pos * 4, // Conservative byte approximation (UTF-8)
                old_end_byte: insert_pos * 4,
                new_end_byte: (insert_pos + inserted_chars) * 4,
                start_position: start_point,
                old_end_position: start_point,
                new_end_position: end_point,
            };
            tree.edit(&edit);
        }

        // Advance cursors ahead of the newly inserted segment
        for cursor in &mut self.cursors {
            if *cursor >= insert_pos {
                *cursor += inserted_chars;
            }
        }

        // Trigger an incremental parse update in the background
        self.reparse_buffer();
    }

    /// Delete a range of characters from the rope, recalculating tree nodes dynamically.
    pub fn delete_range(&mut self, range: Range<usize>) {
        if range.start >= range.end || range.end > self.buffer.len_chars() {
            return;
        }

        let start_point = self.offset_to_point(range.start);
        let end_point = self.offset_to_point(range.end);
        let deleted_chars = range.end - range.start;

        // O(log n) B-Tree deletion
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

        // Align active cursor indexes
        for cursor in &mut self.cursors {
            if *cursor >= range.end {
                *cursor -= deleted_chars;
            } else if *cursor > range.start {
                *cursor = range.start;
            }
        }

        self.reparse_buffer();
    }

    /// Reparses the rope buffer incrementally. Instead of converting the full rope 
    /// back into a flat string, we feed text chunks directly to the Tree-sitter parser,
    /// avoiding heavy string reallocations.
    fn reparse_buffer(&mut self) {
        let rope_ref = &self.buffer;
        let callback = |byte_idx: usize, _point: Point| -> &[u8] {
            let char_idx = byte_idx / 4; // Flat char conversion estimate
            if char_idx >= rope_ref.len_chars() {
                return &[];
            }
            let (slice, _, _, _) = rope_ref.chunk_at_char(char_idx);
            slice.as_bytes()
        };

        // Leverage Tree-sitter's incremental parsing using the previous AST as a baseline
        self.ts_tree = self.ts_parser.parse_with(callback, self.ts_tree.as_ref());
    }

    /// Bind new incoming diagnostic arrays from background LSP threads.
    pub fn update_diagnostics(&mut self, diagnostics: Vec<LspDiagnostic>) {
        self.diagnostics = diagnostics;
    }

    /// Helper: Converts flat linear character offsets into 2D (row, column) Point coordinates.
    fn offset_to_point(&self, offset: usize) -> Point {
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

    // --- High-Performance Getter APIs ---

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
