// src/pane_manager.rs
//! Tree-based Layout & Tiling Pane Manager for Well Terminal

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use well_shell::pty::PtySession;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Clone)]
pub struct TerminalPane {
    pub id: usize,
    pub pty: Arc<PtySession>,
    pub width: f32,
    pub height: f32,
    pub active: bool,
}

impl TerminalPane {
    pub fn new(id: usize, pty: Arc<PtySession>) -> Self {
        Self {
            id,
            pty,
            width: 1.0,
            height: 1.0,
            active: true,
        }
    }
}

pub enum LayoutNode {
    Leaf(TerminalPane),
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}

pub struct PaneManager {
    pub root: LayoutNode,
    pub active_pane_id: usize,
    next_id: AtomicUsize,
}

impl PaneManager {
    pub fn new(initial_pty: Arc<PtySession>) -> Self {
        let pane = TerminalPane::new(0, initial_pty);
        Self {
            root: LayoutNode::Leaf(pane),
            active_pane_id: 0,
            next_id: AtomicUsize::new(1),
        }
    }

    pub fn split(&mut self, direction: SplitDirection, new_pty: Arc<PtySession>) {
        let new_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let new_pane = TerminalPane::new(new_id, new_pty);

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
    }

    pub fn pane_count(&self) -> usize {
        fn count(node: &LayoutNode) -> usize {
            match node {
                LayoutNode::Leaf(_) => 1,
                LayoutNode::Split { first, second, .. } => count(first) + count(second),
            }
        }
        count(&self.root)
    }

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
}
