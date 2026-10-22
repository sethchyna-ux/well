//! A selection owns its visible snapshot so incoming output cannot change copied text.
use well_shell::pty::ScreenSnapshot;

pub struct Selection {
    pub screen: ScreenSnapshot,
    anchor: usize,
    end: usize,
    pub dragging: bool,
    moved: bool,
}
impl Selection {
    pub fn new(mut screen: ScreenSnapshot, row: u16, col: u16) -> Self {
        screen.cursor = None;
        let anchor = row as usize * screen.cols as usize + col as usize;
        Self {
            screen,
            anchor,
            end: anchor,
            dragging: true,
            moved: false,
        }
    }
    pub fn extend(&mut self, row: u16, col: u16) {
        let end = row as usize * self.screen.cols as usize + col as usize;
        self.moved |= end != self.anchor;
        self.end = end;
    }
    pub fn range(&self) -> Option<(usize, usize)> {
        self.moved
            .then(|| (self.anchor.min(self.end), self.anchor.max(self.end)))
    }
    pub fn text(&self) -> Option<String> {
        let (mut first, mut last) = self.range()?;
        // Include the leading cell if the drag starts on a wide continuation.
        if self.screen.cells[first].is_wide_continuation() {
            first = first.saturating_sub(1);
        }
        if self.screen.cells[last].is_wide() {
            last = (last + 1).min(self.screen.cells.len() - 1);
        }
        let cols = self.screen.cols as usize;
        let mut text = String::new();
        for row in first / cols..=last / cols {
            let start = if row == first / cols { first % cols } else { 0 };
            let end = if row == last / cols {
                last % cols
            } else {
                cols - 1
            };
            let mut line = String::new();
            for col in start..=end {
                let cell = &self.screen.cells[row * cols + col];
                if !cell.is_wide_continuation() {
                    if cell.contents().is_empty() {
                        line.push(' ');
                    } else {
                        line.push_str(cell.contents());
                    }
                }
            }
            let wraps = self.screen.wrapped[row] && end == cols - 1;
            if wraps {
                text.push_str(&line);
            } else {
                text.push_str(line.trim_end_matches(' '));
            }
            if row < last / cols && !wraps {
                text.push('\n');
            }
        }
        Some(text)
    }
}
