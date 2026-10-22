//! Orpheus is the canonical font-backed terminal renderer.
//! This module retains the subsystem name without a separate rendering path.
pub use crate::text::{CellMetrics, TextRenderer as OrpheusRenderer};
