//! well-metrics: Prometheus Telemetry Engine for Well Terminal
//!
//! Exposes microsecond-precision render frame timings, PTY read latencies,
//! keystroke statistics, and IPC seqlock collision counters.

use lazy_static::lazy_static;
use prometheus::{histogram_opts, Encoder, Histogram, IntCounter, Registry, TextEncoder};

lazy_static! {
    pub static ref KEYSTROKE_COUNTER: IntCounter = IntCounter::new(
        "well_keystroke_total",
        "Total user keystrokes handled by the terminal event loop"
    ).expect("failed to create keystroke counter");

    pub static ref RENDER_FRAME_TIME_HISTOGRAM: Histogram = Histogram::with_opts(
        histogram_opts!(
            "well_render_frame_duration_seconds",
            "GPU render frame duration in seconds",
            vec![0.0005, 0.001, 0.002, 0.004, 0.008, 0.016, 0.033]
        )
    ).expect("failed to create render frame histogram");

    pub static ref PTY_READ_LATENCY_HISTOGRAM: Histogram = Histogram::with_opts(
        histogram_opts!(
            "well_pty_read_latency_seconds",
            "Latency of reading escape sequences from PTY master stream",
            vec![0.0001, 0.0005, 0.001, 0.005, 0.010, 0.050]
        )
    ).expect("failed to create pty read latency histogram");

    pub static ref LLM_TRANSLATION_DURATION: Histogram = Histogram::with_opts(
        histogram_opts!(
            "well_llm_translation_duration_seconds",
            "Natural language shell command translation duration",
            vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0]
        )
    ).expect("failed to create llm translation duration histogram");

    pub static ref SEQLOCK_COLLISION_COUNTER: IntCounter = IntCounter::new(
        "well_seqlock_collision_total",
        "Total lock-free Seqlock read collisions detected during concurrent IPC"
    ).expect("failed to create seqlock collision counter");

    pub static ref REGISTRY: Registry = {
        let r = Registry::new();
        let _ = r.register(Box::new(KEYSTROKE_COUNTER.clone()));
        let _ = r.register(Box::new(RENDER_FRAME_TIME_HISTOGRAM.clone()));
        let _ = r.register(Box::new(PTY_READ_LATENCY_HISTOGRAM.clone()));
        let _ = r.register(Box::new(LLM_TRANSLATION_DURATION.clone()));
        let _ = r.register(Box::new(SEQLOCK_COLLISION_COUNTER.clone()));
        r
    };
}

/// Initializes and registers Prometheus collectors into the global Well telemetry registry
pub fn init_metrics() {
    // Access REGISTRY to ensure lazy_static registration runs
    let _ = &*REGISTRY;
}

/// Exports all gathered telemetry formatted for Prometheus /metrics scrape endpoints
pub fn export_prometheus_text() -> Result<String, String> {
    init_metrics();
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder
        .encode(&metric_families, &mut buffer)
        .map_err(|e| format!("Failed to encode Prometheus metrics: {}", e))?;
    String::from_utf8(buffer).map_err(|e| format!("Invalid UTF-8 metrics output: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_initialization_and_counters() {
        init_metrics();
        KEYSTROKE_COUNTER.inc();
        KEYSTROKE_COUNTER.inc_by(5);
        assert!(KEYSTROKE_COUNTER.get() >= 6);

        SEQLOCK_COLLISION_COUNTER.inc();
        assert!(SEQLOCK_COLLISION_COUNTER.get() >= 1);

        let exported = export_prometheus_text().expect("export should succeed");
        assert!(exported.contains("well_keystroke_total"));
        assert!(exported.contains("well_seqlock_collision_total"));
    }

    #[test]
    fn test_histograms_observation() {
        init_metrics();
        RENDER_FRAME_TIME_HISTOGRAM.observe(0.0015);
        PTY_READ_LATENCY_HISTOGRAM.observe(0.0003);
        LLM_TRANSLATION_DURATION.observe(0.085);

        let exported = export_prometheus_text().expect("export should succeed");
        assert!(exported.contains("well_render_frame_duration_seconds"));
        assert!(exported.contains("well_pty_read_latency_seconds"));
        assert!(exported.contains("well_llm_translation_duration_seconds"));
    }
}
