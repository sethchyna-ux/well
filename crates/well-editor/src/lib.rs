use ropey::Rope;
use std::ops::Range;

/// A Unicode-safe text buffer for inline command composition.
///
/// Cursor and selection positions are measured in Unicode scalar values.
pub struct InlineEditor {
    buffer: Rope,
    cursor: usize,
    selection: Option<Range<usize>>,
}

impl Default for InlineEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl InlineEditor {
    pub fn new() -> Self {
        Self {
            buffer: Rope::new(),
            cursor: 0,
            selection: None,
        }
    }

    pub fn from_text(text: &str) -> Self {
        let mut editor = Self::new();
        editor.buffer = Rope::from_str(text);
        editor.cursor = editor.buffer.len_chars();
        editor
    }

    pub fn text(&self) -> String {
        self.buffer.to_string()
    }

    pub fn len_chars(&self) -> usize {
        self.buffer.len_chars()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn selection(&self) -> Option<Range<usize>> {
        self.selection.clone()
    }

    pub fn selected_text(&self) -> Option<String> {
        self.selection
            .as_ref()
            .map(|range| self.buffer.slice(range.clone()).to_string())
    }

    pub fn set_cursor(&mut self, position: usize) {
        self.cursor = position.min(self.buffer.len_chars());
        self.selection = None;
    }

    pub fn set_selection(&mut self, range: Range<usize>) {
        let length = self.buffer.len_chars();
        let start = range.start.min(length);
        let end = range.end.min(length);
        self.selection = (start != end).then_some(start.min(end)..start.max(end));
        self.cursor = end;
    }

    pub fn insert(&mut self, text: &str) {
        self.delete_selection();
        self.buffer.insert(self.cursor, text);
        self.cursor += text.chars().count();
    }

    pub fn delete_backward(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        if self.cursor == 0 {
            return false;
        }

        self.buffer.remove(self.cursor - 1..self.cursor);
        self.cursor -= 1;
        true
    }

    pub fn delete_forward(&mut self) -> bool {
        if self.delete_selection() {
            return true;
        }
        if self.cursor == self.buffer.len_chars() {
            return false;
        }

        self.buffer.remove(self.cursor..self.cursor + 1);
        true
    }

    fn delete_selection(&mut self) -> bool {
        let Some(range) = self.selection.take() else {
            return false;
        };
        self.buffer.remove(range.clone());
        self.cursor = range.start;
        true
    }
}

/// Initialize an empty inline editor.
pub fn init_editor() -> InlineEditor {
    InlineEditor::new()
}

#[cfg(test)]
mod tests {
    use super::InlineEditor;
    use std::ops::Range;

    #[test]
    fn edits_unicode_text_by_character_position() {
        let mut editor = InlineEditor::from_text("aé🙂");
        assert_eq!(editor.len_chars(), 3);
        editor.set_cursor(2);
        editor.insert("界");
        assert_eq!(editor.text(), "aé界🙂");

        assert!(editor.delete_backward());
        assert_eq!(editor.text(), "aé🙂");
        assert!(editor.delete_forward());
        assert_eq!(editor.text(), "aé");
    }

    #[test]
    fn replaces_selected_range_and_normalizes_reversed_selection() {
        let mut editor = InlineEditor::from_text("hello world");
        editor.set_selection(Range { start: 9, end: 6 });
        assert_eq!(editor.selection(), Some(6..9));
        assert_eq!(editor.selected_text().as_deref(), Some("wor"));

        editor.insert("you");
        assert_eq!(editor.text(), "hello yould");
        assert_eq!(editor.cursor(), 9);
        assert_eq!(editor.selection(), None);
    }

    #[test]
    fn clamps_ranges_and_handles_empty_buffer_edges() {
        let mut editor = InlineEditor::new();
        assert!(!editor.delete_backward());
        assert!(!editor.delete_forward());

        editor.insert("ok");
        editor.set_selection(1..usize::MAX);
        assert_eq!(editor.selected_text().as_deref(), Some("k"));
        assert!(editor.delete_backward());
        assert_eq!(editor.text(), "o");
        assert_eq!(editor.cursor(), 1);
    }
}
