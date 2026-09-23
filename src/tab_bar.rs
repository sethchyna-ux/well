//! Multi-Session Tab Bar & Workspace Tab Model for Well Terminal
//!
//! Subsystems: ATLAS / THEIA
//! Manages multiple terminal workspaces (tabs), each containing an independent
//! tiling layout tree (`PaneManager`). Provides tab switching, lifecycle operations,
//! and egui tab strip representation.

use crate::pane_manager::{PaneManager, SplitDirection};
use std::sync::Arc;
use well_shell::pty::PtySession;

/// A single workspace tab containing its own tiling tree of terminal panes.
pub struct Tab {
    pub id: usize,
    pub title: String,
    pub pane_manager: PaneManager,
}

impl Tab {
    pub fn new(id: usize, title: Option<String>, initial_pty: Arc<PtySession>) -> Self {
        Self {
            id,
            title: title.unwrap_or_else(|| format!("Tab {}", id + 1)),
            pane_manager: PaneManager::new(initial_pty),
        }
    }
}

/// Manages a collection of workspace tabs.
pub struct TabBar {
    pub tabs: Vec<Tab>,
    pub active_tab_index: usize,
    next_tab_id: usize,
}

impl TabBar {
    /// Initializes the tab bar with a single initial tab.
    pub fn new(initial_pty: Arc<PtySession>) -> Self {
        let initial_tab = Tab::new(0, Some("Workspace 1".to_string()), initial_pty);
        Self {
            tabs: vec![initial_tab],
            active_tab_index: 0,
            next_tab_id: 1,
        }
    }

    /// Spawns a new tab with the given PTY session.
    /// Returns the ID of the new tab.
    pub fn new_tab(&mut self, pty: Arc<PtySession>, title: Option<String>) -> usize {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let tab = Tab::new(id, title, pty);
        self.tabs.push(tab);
        self.active_tab_index = self.tabs.len() - 1;
        id
    }

    /// Returns a reference to the active tab.
    pub fn active_tab(&self) -> &Tab {
        &self.tabs[self.active_tab_index]
    }

    /// Returns a mutable reference to the active tab.
    pub fn active_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active_tab_index]
    }

    /// Selects the tab at the given index.
    pub fn select_tab(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.active_tab_index = index;
            true
        } else {
            false
        }
    }

    /// Closes the tab at the specified index.
    /// Returns true if closed, false if it is the only remaining tab.
    pub fn close_tab(&mut self, index: usize) -> bool {
        if self.tabs.len() <= 1 || index >= self.tabs.len() {
            return false;
        }

        self.tabs.remove(index);
        if self.active_tab_index >= self.tabs.len() {
            self.active_tab_index = self.tabs.len() - 1;
        }
        true
    }

    /// Closes the currently active tab.
    pub fn close_active_tab(&mut self) -> bool {
        self.close_tab(self.active_tab_index)
    }

    /// Cycles to the next tab.
    pub fn next_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.active_tab_index = (self.active_tab_index + 1) % self.tabs.len();
        }
    }

    /// Cycles to the previous tab.
    pub fn prev_tab(&mut self) {
        if !self.tabs.is_empty() {
            if self.active_tab_index == 0 {
                self.active_tab_index = self.tabs.len() - 1;
            } else {
                self.active_tab_index -= 1;
            }
        }
    }

    /// Returns the active PTY session for the active pane in the active tab.
    pub fn active_pty(&self) -> Option<Arc<PtySession>> {
        self.active_tab().pane_manager.active_pty()
    }

    /// Returns all PTY sessions across all tabs and all panes.
    pub fn all_ptys(&self) -> Vec<(usize, Arc<PtySession>)> {
        let mut ptys = Vec::new();
        for tab in &self.tabs {
            ptys.extend(tab.pane_manager.all_ptys());
        }
        ptys
    }

    /// Resizes all active PTY sessions across all tabs and panes.
    pub fn resize_all(&self, rows: u16, cols: u16) {
        for (_, pty) in self.all_ptys() {
            let _ = pty.resize(rows, cols);
        }
    }

    /// Splits the active pane in the active tab.
    pub fn split_active_pane(
        &mut self,
        direction: SplitDirection,
        new_pty: Arc<PtySession>,
    ) -> usize {
        self.active_tab_mut().pane_manager.split(direction, new_pty)
    }

    /// Closes the active pane in the active tab, or closes the tab if it only has one pane.
    pub fn close_active_pane_or_tab(&mut self) -> bool {
        let pane_count = self.active_tab().pane_manager.pane_count();
        if pane_count > 1 {
            let active_id = self.active_tab().pane_manager.active_pane_id;
            self.active_tab_mut().pane_manager.close(active_id)
        } else {
            self.close_active_tab()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_bar_lifecycle() {
        let pty1 = Arc::new(PtySession::spawn(24, 80, || {}).expect("PTY 1"));
        let mut tb = TabBar::new(Arc::clone(&pty1));
        assert_eq!(tb.tabs.len(), 1);
        assert_eq!(tb.active_tab_index, 0);

        let pty2 = Arc::new(PtySession::spawn(24, 80, || {}).expect("PTY 2"));
        let tab2_id = tb.new_tab(Arc::clone(&pty2), Some("Logs".to_string()));
        assert_eq!(tb.tabs.len(), 2);
        assert_eq!(tb.active_tab_index, 1);
        assert_eq!(tb.active_tab().id, tab2_id);

        // Split active pane in tab 2
        let pty3 = Arc::new(PtySession::spawn(24, 80, || {}).expect("PTY 3"));
        tb.split_active_pane(SplitDirection::Horizontal, Arc::clone(&pty3));
        assert_eq!(tb.active_tab().pane_manager.pane_count(), 2);

        // Navigation
        tb.next_tab();
        assert_eq!(tb.active_tab_index, 0);
        tb.prev_tab();
        assert_eq!(tb.active_tab_index, 1);

        // Close pane or tab
        assert!(tb.close_active_pane_or_tab()); // Should close split pane first
        assert_eq!(tb.active_tab().pane_manager.pane_count(), 1);
        assert_eq!(tb.tabs.len(), 2);

        assert!(tb.close_active_pane_or_tab()); // Should close tab 2
        assert_eq!(tb.tabs.len(), 1);
        assert_eq!(tb.active_tab_index, 0);

        // Last tab cannot be closed
        assert!(!tb.close_active_pane_or_tab());
        assert_eq!(tb.tabs.len(), 1);
    }
}
