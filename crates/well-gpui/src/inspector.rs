//! AgentOS bare-metal framebuffer tile diffing and telemetry sparkline aggregation.
//!
//! Used by Well GPUI's live AgentOS inspector to stream only modified framebuffer
//! regions over shared memory / VNC / VirtIO-GPU and plot 29-column THEIA telemetry
//! windows without heap thrashing.

use serde::{Deserialize, Serialize};

/// Canonical field count for THEIA 29-column BigQuery-compatible NDJSON telemetry.
pub const TELEMETRY_CONTRACT_FIELD_COUNT: usize = 29;

/// Mirror of `kernel_rust::theia::contract::AgentOsTelemetryRecord` for zero-copy
/// NDJSON deserialization inside the Well desktop inspector.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AgentOsTelemetryRecord {
    // 1..5: Identity & Causality
    pub schema_version: u16,
    pub timestamp_unix_ns: u64,
    pub boot_session_id: u64,
    pub trace_id: u64,
    pub span_id: u64,
    // 6..10: CPU & TALOS Scheduler
    pub cpu_core_id: u16,
    pub talos_task_id: u64,
    pub talos_priority: u8,
    pub talos_fuel_consumed: u64,
    pub talos_context_switches: u64,
    // 11..15: HERMES IPC Ring Metrics
    pub hermes_channel_id: u32,
    pub hermes_tx_frames: u64,
    pub hermes_rx_frames: u64,
    pub hermes_ring_depth: u32,
    pub hermes_kicks_suppressed: u64,
    // 16..19: MNEMOSYNE Memory & Cache
    pub mnemosyne_used_pages: u32,
    pub mnemosyne_pinned_pages: u32,
    pub mnemosyne_cache_hits: u64,
    pub mnemosyne_evictions: u64,
    // 20..22: ORPHEUS DAG & WAL
    pub orpheus_last_lsn: u64,
    pub orpheus_dag_nodes: u32,
    pub orpheus_wal_crc_errors: u32,
    // 23..26: PYTHIA & ARGUS Security
    pub pythia_attest_status: u8,
    pub pythia_sev_snp_active: bool,
    pub argus_cap_checks_ok: u64,
    pub argus_cap_denials: u32,
    // 27..29: MicroVM Hardware Health
    pub argus_redactions_count: u32,
    pub microvm_irq_count: u64,
    pub microvm_entropy_bits: u32,
}

/// Rectangular dirty region identified by FNV-1a tile hashing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirtyTileRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub fnv1a_hash: u64,
}

/// Tracks BGRA framebuffer contents by fixed-size tile FNV-1a digests so unchanged
/// tiles are skipped during GPU texture uploads.
#[derive(Debug, Clone)]
pub struct FramebufferTileTracker {
    fb_width: u32,
    fb_height: u32,
    tile_size: u32,
    tiles_x: u32,
    tiles_y: u32,
    tile_hashes: Vec<u64>,
    tile_valid: Vec<bool>,
}

impl FramebufferTileTracker {
    const FNV1A_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV1A_PRIME: u64 = 0x0100_0000_01b3;

    /// Constructs a tracker for a framebuffer of `fb_width x fb_height` pixels
    /// partitioned into square tiles of side `tile_size` (clamped to `>= 1`).
    pub fn new(fb_width: u32, fb_height: u32, tile_size: u32) -> Self {
        let tile_size = tile_size.max(1);
        let tiles_x = if fb_width == 0 {
            0
        } else {
            fb_width.div_ceil(tile_size)
        };
        let tiles_y = if fb_height == 0 {
            0
        } else {
            fb_height.div_ceil(tile_size)
        };
        let total_tiles = (tiles_x as usize).saturating_mul(tiles_y as usize);
        Self {
            fb_width,
            fb_height,
            tile_size,
            tiles_x,
            tiles_y,
            tile_hashes: vec![0; total_tiles],
            tile_valid: vec![false; total_tiles],
        }
    }

    /// Returns `(tiles_x, tiles_y)` grid dimensions.
    pub fn tile_grid(&self) -> (u32, u32) {
        (self.tiles_x, self.tiles_y)
    }

    /// Resets cached digests so the next `compute_dirty_tiles` call emits the full grid.
    pub fn invalidate_all(&mut self) {
        self.tile_valid.fill(false);
    }

    /// Computes FNV-1a hashes across all tiles of `bgra_pixels` (row-major `fb_width * fb_height`)
    /// and returns only tiles whose hash changed since the previous frame.
    ///
    /// Returns an empty vector if dimensions are zero or `bgra_pixels` is smaller than
    /// `fb_width * fb_height`.
    pub fn compute_dirty_tiles(&mut self, bgra_pixels: &[u32]) -> Vec<DirtyTileRect> {
        let expected_pixels = (self.fb_width as usize).saturating_mul(self.fb_height as usize);
        if expected_pixels == 0 || bgra_pixels.len() < expected_pixels {
            return Vec::new();
        }

        let mut dirty = Vec::new();
        let stride = self.fb_width as usize;

        for ty in 0..self.tiles_y {
            let y0 = ty * self.tile_size;
            let height = (self.fb_height - y0).min(self.tile_size);

            for tx in 0..self.tiles_x {
                let x0 = tx * self.tile_size;
                let width = (self.fb_width - x0).min(self.tile_size);

                let hash = Self::hash_tile(bgra_pixels, stride, x0, y0, width, height);
                let tile_idx = (ty as usize) * (self.tiles_x as usize) + (tx as usize);

                if !self.tile_valid[tile_idx] || self.tile_hashes[tile_idx] != hash {
                    self.tile_hashes[tile_idx] = hash;
                    self.tile_valid[tile_idx] = true;
                    dirty.push(DirtyTileRect {
                        x: x0,
                        y: y0,
                        width,
                        height,
                        fnv1a_hash: hash,
                    });
                }
            }
        }

        dirty
    }

    fn hash_tile(pixels: &[u32], stride: usize, x0: u32, y0: u32, w: u32, h: u32) -> u64 {
        let mut hash = Self::FNV1A_OFFSET_BASIS;
        for row in 0..h {
            let row_start = ((y0 + row) as usize) * stride + (x0 as usize);
            let row_slice = &pixels[row_start..row_start + (w as usize)];
            for &pixel in row_slice {
                for byte in pixel.to_le_bytes() {
                    hash ^= u64::from(byte);
                    hash = hash.wrapping_mul(Self::FNV1A_PRIME);
                }
            }
        }
        hash
    }
}

/// Fixed-capacity sliding window of AgentOS THEIA telemetry records used for
/// sub-millisecond inspector sparkline rendering.
#[derive(Debug, Clone)]
pub struct TelemetrySparklineWindow<const SAMPLES: usize> {
    records: Vec<AgentOsTelemetryRecord>,
    head: usize,
}

impl<const SAMPLES: usize> Default for TelemetrySparklineWindow<SAMPLES> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SAMPLES: usize> TelemetrySparklineWindow<SAMPLES> {
    /// Creates an empty sparkline window retaining at most `SAMPLES` records.
    pub fn new() -> Self {
        Self {
            records: Vec::with_capacity(SAMPLES),
            head: 0,
        }
    }

    /// Number of samples currently stored in the window (`<= SAMPLES`).
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns true if no telemetry samples have been pushed yet.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Pushes a telemetry record, overwriting the oldest sample once capacity is reached.
    pub fn push_record(&mut self, rec: &AgentOsTelemetryRecord) {
        if SAMPLES == 0 {
            return;
        }
        if self.records.len() < SAMPLES {
            self.records.push(rec.clone());
        } else {
            self.records[self.head] = rec.clone();
            self.head = (self.head + 1) % SAMPLES;
        }
    }

    /// Parses one BigQuery-compatible NDJSON telemetry line and pushes it into the window.
    pub fn push_ndjson_line(&mut self, line: &str) -> Result<(), serde_json::Error> {
        let rec: AgentOsTelemetryRecord = serde_json::from_str(line)?;
        self.push_record(&rec);
        Ok(())
    }

    /// Returns the most recently pushed telemetry record, if any.
    pub fn latest(&self) -> Option<&AgentOsTelemetryRecord> {
        if self.records.is_empty() {
            return None;
        }
        if self.records.len() < SAMPLES {
            self.records.last()
        } else {
            let newest = (self.head + SAMPLES - 1) % SAMPLES;
            self.records.get(newest)
        }
    }

    /// Iterates over stored records strictly in chronological (oldest -> newest) order.
    pub fn ordered_records(&self) -> impl Iterator<Item = &AgentOsTelemetryRecord> {
        let split = if self.records.len() < SAMPLES {
            0
        } else {
            self.head
        };
        let (tail, front) = self.records.split_at(split);
        front.iter().chain(tail.iter())
    }

    /// Normalizes `talos_fuel_consumed` values in chronological order into `[0.0, 1.0]`.
    pub fn normalized_fuel_points(&self) -> Vec<f32> {
        self.normalize_u64_metric(|r| r.talos_fuel_consumed)
    }

    /// Normalizes `hermes_ring_depth` values in chronological order into `[0.0, 1.0]`.
    pub fn normalized_ring_depth_points(&self) -> Vec<f32> {
        self.normalize_u64_metric(|r| u64::from(r.hermes_ring_depth))
    }

    fn normalize_u64_metric<F>(&self, extractor: F) -> Vec<f32>
    where
        F: Fn(&AgentOsTelemetryRecord) -> u64,
    {
        if self.records.is_empty() {
            return Vec::new();
        }
        let mut min_val = u64::MAX;
        let mut max_val = u64::MIN;
        let mut raw = Vec::with_capacity(self.records.len());

        for rec in self.ordered_records() {
            let v = extractor(rec);
            min_val = min_val.min(v);
            max_val = max_val.max(v);
            raw.push(v);
        }

        if max_val == min_val {
            let constant = if max_val == 0 { 0.0 } else { 1.0 };
            return vec![constant; raw.len()];
        }

        let span = (max_val - min_val) as f64;
        raw.into_iter()
            .map(|v| (((v - min_val) as f64) / span) as f32)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_framebuffer_tile_tracker_detects_single_dirty_tile() {
        let width = 16u32;
        let height = 16u32;
        let tile = 8u32;
        let mut fb = vec![0xFF11_2233u32; (width * height) as usize];
        let mut tracker = FramebufferTileTracker::new(width, height, tile);
        assert_eq!(tracker.tile_grid(), (2, 2));

        // Initial frame: all 4 tiles are dirty.
        let initial_dirty = tracker.compute_dirty_tiles(&fb);
        assert_eq!(initial_dirty.len(), 4);

        // Unchanged frame: 0 dirty tiles.
        let clean_dirty = tracker.compute_dirty_tiles(&fb);
        assert!(clean_dirty.is_empty());

        // Mutate a pixel inside tile (1, 1) -> (x=10, y=12).
        fb[12 * (width as usize) + 10] = 0xFFAA_BBCC;
        let delta_dirty = tracker.compute_dirty_tiles(&fb);
        assert_eq!(delta_dirty.len(), 1);
        assert_eq!(delta_dirty[0].x, 8);
        assert_eq!(delta_dirty[0].y, 8);
        assert_eq!(delta_dirty[0].width, 8);
        assert_eq!(delta_dirty[0].height, 8);
    }

    #[test]
    fn test_framebuffer_tile_tracker_handles_non_multiple_dimensions_and_short_buffer() {
        let mut tracker = FramebufferTileTracker::new(10, 9, 8);
        assert_eq!(tracker.tile_grid(), (2, 2));

        // Short buffer returns empty cleanly rather than panicking.
        assert!(tracker.compute_dirty_tiles(&[0u32; 10]).is_empty());

        let fb = vec![0x1234_5678u32; 90];
        let dirty = tracker.compute_dirty_tiles(&fb);
        assert_eq!(dirty.len(), 4);
        assert_eq!((dirty[3].x, dirty[3].y, dirty[3].width, dirty[3].height), (8, 8, 2, 1));
    }

    #[test]
    fn test_telemetry_sparkline_window_wraparound_and_normalization() {
        let mut window: TelemetrySparklineWindow<4> = TelemetrySparklineWindow::new();
        assert!(window.is_empty());

        for step in 1..=6u64 {
            let rec = AgentOsTelemetryRecord {
                schema_version: 1,
                timestamp_unix_ns: step * 1_000,
                talos_fuel_consumed: step * 100,
                hermes_ring_depth: (step as u32) * 2,
                ..AgentOsTelemetryRecord::default()
            };
            window.push_record(&rec);
        }

        // Retained samples must be steps [3, 4, 5, 6] => fuel [300, 400, 500, 600].
        assert_eq!(window.len(), 4);
        assert_eq!(window.latest().map(|r| r.talos_fuel_consumed), Some(600));

        let fuel_pts = window.normalized_fuel_points();
        assert_eq!(fuel_pts.len(), 4);
        assert!((fuel_pts[0] - 0.0).abs() < 1e-6);
        assert!((fuel_pts[1] - (1.0 / 3.0)).abs() < 1e-5);
        assert!((fuel_pts[2] - (2.0 / 3.0)).abs() < 1e-5);
        assert!((fuel_pts[3] - 1.0).abs() < 1e-6);

        // Verify NDJSON ingestion matches all 29 contract columns.
        let sample_json = serde_json::to_string(&AgentOsTelemetryRecord {
            schema_version: 2,
            talos_fuel_consumed: 900,
            pythia_sev_snp_active: true,
            ..AgentOsTelemetryRecord::default()
        });
        assert!(sample_json.is_ok());
        if let Ok(line) = sample_json {
            assert!(window.push_ndjson_line(&line).is_ok());
            assert_eq!(window.latest().map(|r| r.talos_fuel_consumed), Some(900));
            assert_eq!(window.latest().map(|r| r.pythia_sev_snp_active), Some(true));
        }
    }
}
