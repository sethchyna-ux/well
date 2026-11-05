//! Tree-based Layout & Tiling Pane Manager for Well Terminal
//!
//! Subsystems: ATLAS / METIS
//! Provides binary-tree tiling, rect slicing, keyboard/mouse focus routing,
//! and graceful pane closing with tree consolidation.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use well_shell::pty::PtySession;

/// Direction for tiling splits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// Slices vertically, creating left and right sub-panes.
    Horizontal,
    /// Slices horizontally, creating top and bottom sub-panes.
    Vertical,
}

/// A single terminal pane leaf node.
#[derive(Clone)]
pub struct TerminalPane {
    pub id: usize,
    pub pty: Arc<PtySession>,
    pub title: String,
}

impl TerminalPane {
    pub fn new(id: usize, pty: Arc<PtySession>, title: Option<String>) -> Self {
        Self {
            id,
            pty,
            title: title.unwrap_or_else(|| format!("Terminal {id}")),
        }
    }
}

/// Binary tree node representing either a leaf pane or a split.
pub enum LayoutNode {
    Leaf(TerminalPane),
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}

/// 2D Rectangle (x, y, width, height) in physical or logical coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaneRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl PaneRect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= (self.x + self.width) && py >= self.y && py <= (self.y + self.height)
    }
}

/// Manages a tiling layout tree of terminal panes.
pub struct PaneManager {
    pub root: LayoutNode,
    pub active_pane_id: usize,
    next_id: AtomicUsize,
}

impl PaneManager {
    /// Initialize with a single root pane.
    pub fn new(initial_pty: Arc<PtySession>) -> Self {
        let pane = TerminalPane::new(0, initial_pty, Some("Main".to_string()));
        Self {
            root: LayoutNode::Leaf(pane),
            active_pane_id: 0,
            next_id: AtomicUsize::new(1),
        }
    }

    /// Splits the currently active pane in the specified direction.
    /// Returns the ID of the newly spawned pane.
    pub fn split(&mut self, direction: SplitDirection, new_pty: Arc<PtySession>) -> usize {
        let new_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let new_pane = TerminalPane::new(new_id, new_pty, None);

        fn split_node(
            node: &mut LayoutNode,
            active_id: usize,
            direction: SplitDirection,
            new_pane: TerminalPane,
        ) -> bool {
            match node {
                LayoutNode::Leaf(pane) if pane.id == active_id => {
                    let old_pane = pane.clone();
                    *node = LayoutNode::Split {
                        direction,
                        ratio: 0.5,
                        first: Box::new(LayoutNode::Leaf(old_pane)),
                        second: Box::new(LayoutNode::Leaf(new_pane)),
                    };
                    true
                }
                LayoutNode::Split { first, second, .. } => {
                    split_node(first, active_id, direction, new_pane.clone())
                        || split_node(second, active_id, direction, new_pane)
                }
                _ => false,
            }
        }

        if split_node(&mut self.root, self.active_pane_id, direction, new_pane) {
            self.active_pane_id = new_id;
        }

        new_id
    }

    /// Closes the pane with the specified ID.
    /// Consolidates the parent split node if a sibling remains.
    /// Returns true if the pane was found and closed, false if it is the only remaining pane.
    pub fn close(&mut self, target_id: usize) -> bool {
        if self.pane_count() <= 1 {
            return false;
        }

        fn remove_node(node: LayoutNode, target_id: usize) -> Result<LayoutNode, LayoutNode> {
            match node {
                LayoutNode::Leaf(pane) => {
                    if pane.id == target_id {
                        Err(LayoutNode::Leaf(pane))
                    } else {
                        Ok(LayoutNode::Leaf(pane))
                    }
                }
                LayoutNode::Split {
                    direction,
                    ratio,
                    first,
                    second,
                } => match remove_node(*first, target_id) {
                    Ok(new_first) => match remove_node(*second, target_id) {
                        Ok(new_second) => Ok(LayoutNode::Split {
                            direction,
                            ratio,
                            first: Box::new(new_first),
                            second: Box::new(new_second),
                        }),
                        Err(_) => Ok(new_first),
                    },
                    Err(_) => Ok(*second),
                },
            }
        }

        let dummy = LayoutNode::Leaf(TerminalPane::new(
            usize::MAX,
            Arc::clone(&self.active_pty().unwrap()),
            None,
        ));
        let old_root = std::mem::replace(&mut self.root, dummy);

        match remove_node(old_root, target_id) {
            Ok(new_root) => {
                self.root = new_root;
                if self.active_pane_id == target_id {
                    let all = self.all_pane_ids();
                    if let Some(&first_id) = all.first() {
                        self.active_pane_id = first_id;
                    }
                }
                true
            }
            Err(original_root) => {
                self.root = original_root;
                false
            }
        }
    }

    /// Returns the total number of open terminal panes in this layout tree.
    pub fn pane_count(&self) -> usize {
        fn count(node: &LayoutNode) -> usize {
            match node {
                LayoutNode::Leaf(_) => 1,
                LayoutNode::Split { first, second, .. } => count(first) + count(second),
            }
        }
        count(&self.root)
    }

    /// Computes physical screen bounds for each pane given the total available area.
    pub fn compute_pane_rects(&self, total_bounds: PaneRect) -> Vec<(usize, PaneRect)> {
        let mut rects = Vec::new();

        fn recurse(node: &LayoutNode, rect: PaneRect, out: &mut Vec<(usize, PaneRect)>) {
            match node {
                LayoutNode::Leaf(pane) => {
                    out.push((pane.id, rect));
                }
                LayoutNode::Split {
                    direction,
                    ratio,
                    first,
                    second,
                } => {
                    let r = ratio.clamp(0.05, 0.95);
                    match direction {
                        SplitDirection::Horizontal => {
                            let w1 = (rect.width * r).floor();
                            let w2 = rect.width - w1;
                            let r1 = PaneRect::new(rect.x, rect.y, w1, rect.height);
                            let r2 = PaneRect::new(rect.x + w1, rect.y, w2, rect.height);
                            recurse(first, r1, out);
                            recurse(second, r2, out);
                        }
                        SplitDirection::Vertical => {
                            let h1 = (rect.height * r).floor();
                            let h2 = rect.height - h1;
                            let r1 = PaneRect::new(rect.x, rect.y, rect.width, h1);
                            let r2 = PaneRect::new(rect.x, rect.y + h1, rect.width, h2);
                            recurse(first, r1, out);
                            recurse(second, r2, out);
                        }
                    }
                }
            }
        }

        recurse(&self.root, total_bounds, &mut rects);
        rects
    }

    /// Finds the pane ID located at the given coordinates (px, py).
    pub fn find_pane_at(&self, total_bounds: PaneRect, px: f32, py: f32) -> Option<usize> {
        let rects = self.compute_pane_rects(total_bounds);
        rects
            .into_iter()
            .find(|(_, rect)| rect.contains(px, py))
            .map(|(id, _)| id)
    }

    /// Returns the active pane's PTY session.
    pub fn active_pty(&self) -> Option<Arc<PtySession>> {
        fn find_pty(node: &LayoutNode, target_id: usize) -> Option<Arc<PtySession>> {
            match node {
                LayoutNode::Leaf(pane) if pane.id == target_id => Some(Arc::clone(&pane.pty)),
                LayoutNode::Split { first, second, .. } => {
                    find_pty(first, target_id).or_else(|| find_pty(second, target_id))
                }
                _ => None,
            }
        }
        find_pty(&self.root, self.active_pane_id)
    }

    /// Returns all open pane IDs and their associated PTY sessions.
    pub fn all_ptys(&self) -> Vec<(usize, Arc<PtySession>)> {
        let mut ptys = Vec::new();
        fn collect(node: &LayoutNode, out: &mut Vec<(usize, Arc<PtySession>)>) {
            match node {
                LayoutNode::Leaf(pane) => out.push((pane.id, Arc::clone(&pane.pty))),
                LayoutNode::Split { first, second, .. } => {
                    collect(first, out);
                    collect(second, out);
                }
            }
        }
        collect(&self.root, &mut ptys);
        ptys
    }

    /// Returns all pane IDs in order.
    pub fn all_pane_ids(&self) -> Vec<usize> {
        let mut ids = Vec::new();
        fn collect_ids(node: &LayoutNode, out: &mut Vec<usize>) {
            match node {
                LayoutNode::Leaf(pane) => out.push(pane.id),
                LayoutNode::Split { first, second, .. } => {
                    collect_ids(first, out);
                    collect_ids(second, out);
                }
            }
        }
        collect_ids(&self.root, &mut ids);
        ids
    }

    /// Cycles focus to the next pane.
    pub fn next_pane(&mut self) {
        let ids = self.all_pane_ids();
        if ids.is_empty() {
            return;
        }
        if let Some(pos) = ids.iter().position(|&id| id == self.active_pane_id) {
            let next_pos = (pos + 1) % ids.len();
            self.active_pane_id = ids[next_pos];
        } else {
            self.active_pane_id = ids[0];
        }
    }

    /// Cycles focus to the previous pane.
    pub fn prev_pane(&mut self) {
        let ids = self.all_pane_ids();
        if ids.is_empty() {
            return;
        }
        if let Some(pos) = ids.iter().position(|&id| id == self.active_pane_id) {
            let prev_pos = if pos == 0 { ids.len() - 1 } else { pos - 1 };
            self.active_pane_id = ids[prev_pos];
        } else {
            self.active_pane_id = ids[0];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pane_manager_lifecycle_and_splits() {
        let pty1 = Arc::new(PtySession::spawn(24, 80, || {}).expect("PTY 1"));
        let mut pm = PaneManager::new(Arc::clone(&pty1));
        assert_eq!(pm.pane_count(), 1);
        assert_eq!(pm.active_pane_id, 0);

        let pty2 = Arc::new(PtySession::spawn(24, 80, || {}).expect("PTY 2"));
        let id2 = pm.split(SplitDirection::Horizontal, Arc::clone(&pty2));
        assert_eq!(pm.pane_count(), 2);
        assert_eq!(pm.active_pane_id, id2);

        let pty3 = Arc::new(PtySession::spawn(24, 80, || {}).expect("PTY 3"));
        let id3 = pm.split(SplitDirection::Vertical, Arc::clone(&pty3));
        assert_eq!(pm.pane_count(), 3);
        assert_eq!(pm.active_pane_id, id3);

        let bounds = PaneRect::new(0.0, 0.0, 1000.0, 800.0);
        let rects = pm.compute_pane_rects(bounds);
        assert_eq!(rects.len(), 3);

        // Verify total area coverage
        let total_area: f32 = rects.iter().map(|(_, r)| r.width * r.height).sum();
        assert!((total_area - 800_000.0).abs() < 1.0);

        // Test focus navigation
        pm.next_pane();
        assert_eq!(pm.active_pane_id, 0);
        pm.prev_pane();
        assert_eq!(pm.active_pane_id, id3);

        // Test point detection
        let found = pm.find_pane_at(bounds, 100.0, 100.0);
        assert!(found.is_some());

        // Test closing panes
        assert!(pm.close(id3));
        assert_eq!(pm.pane_count(), 2);

        assert!(pm.close(id2));
        assert_eq!(pm.pane_count(), 1);

        // Closing last pane should return false
        assert!(!pm.close(0));
        assert_eq!(pm.pane_count(), 1);
    }
}
