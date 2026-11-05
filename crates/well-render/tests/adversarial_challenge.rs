//! Comprehensive adversarial challenge test suite for crates/well-render.
//! Stress-tests Orpheus APIs (color conversions, text extraction, cell building)
//! and multi-threaded lock-free ring buffer concurrency.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use well_ipc::ring_buffer::RingBuffer;
use well_ipc::TypedBlock;
use well_render::{
    extract_text_from_screen, vt100_color_to_rgb, CellBuildOptions, OrpheusRenderer,
    SelectionRange,
};

// =========================================================================
// 1. Adversarial Tests for vt100_color_to_rgb
// =========================================================================

#[test]
fn test_vt100_color_to_rgb_all_256_indices_valid_12bit() {
    for i in 0..=255u8 {
        let rgb = vt100_color_to_rgb(vt100::Color::Idx(i), 0x000);
        // Must strictly fit in 12-bit RGB (<= 0x0FFF)
        assert!(
            rgb <= 0x0FFF,
            "Color index {i} produced {rgb:#06X}, exceeding 12 bits"
        );
    }
}

#[test]
fn test_vt100_color_to_rgb_boundary_indices() {
    // Index 0: Black
    assert_eq!(vt100_color_to_rgb(vt100::Color::Idx(0), 0x123), 0x000);
    // Index 1: Red cyber accent
    assert_eq!(vt100_color_to_rgb(vt100::Color::Idx(1), 0x123), 0xF08);
    // Index 7: White
    assert_eq!(vt100_color_to_rgb(vt100::Color::Idx(7), 0x123), 0xCCC);
    // Index 8: Bright Black
    assert_eq!(vt100_color_to_rgb(vt100::Color::Idx(8), 0x123), 0x888);
    // Boundary 15: ANSI Bright White
    assert_eq!(vt100_color_to_rgb(vt100::Color::Idx(15), 0x123), 0xFFF);

    // Boundary 16: Start of 6x6x6 color cube (step 0, 0, 0)
    assert_eq!(vt100_color_to_rgb(vt100::Color::Idx(16), 0x123), 0x000);

    // Mid-cube: Index 196 (step 5, 0, 0 -> bright red)
    assert_eq!(vt100_color_to_rgb(vt100::Color::Idx(196), 0x123), 0xF00);

    // Boundary 231: End of 6x6x6 color cube (step 5, 5, 5 -> bright white)
    assert_eq!(vt100_color_to_rgb(vt100::Color::Idx(231), 0x123), 0xFFF);

    // Boundary 232: Start of 24-step grayscale ramp (value 0 -> black)
    assert_eq!(vt100_color_to_rgb(vt100::Color::Idx(232), 0x123), 0x000);

    // Boundary 255: End of 24-step grayscale ramp (xterm standard ramp 238/255 -> 0xEEE)
    assert_eq!(vt100_color_to_rgb(vt100::Color::Idx(255), 0x123), 0xEEE);
}

#[test]
fn test_vt100_color_to_rgb_default_and_edge_rgb_values() {
    // Default color fallback
    assert_eq!(
        vt100_color_to_rgb(vt100::Color::Default, 0x000),
        0x000,
        "Default with 0x000"
    );
    assert_eq!(
        vt100_color_to_rgb(vt100::Color::Default, 0xFFF),
        0xFFF,
        "Default with 0xFFF"
    );
    assert_eq!(
        vt100_color_to_rgb(vt100::Color::Default, 0x5A3),
        0x5A3,
        "Default with custom pattern"
    );

    // Extreme 24-bit RGB inputs downsampled to 12-bit
    assert_eq!(
        vt100_color_to_rgb(vt100::Color::Rgb(0, 0, 0), 0x000),
        0x000,
        "Pure black RGB"
    );
    assert_eq!(
        vt100_color_to_rgb(vt100::Color::Rgb(255, 255, 255), 0x000),
        0xFFF,
        "Pure white RGB"
    );
    assert_eq!(
        vt100_color_to_rgb(vt100::Color::Rgb(255, 0, 0), 0x000),
        0xF00,
        "Max red RGB"
    );
    assert_eq!(
        vt100_color_to_rgb(vt100::Color::Rgb(0, 255, 0), 0x000),
        0x0F0,
        "Max green RGB"
    );
    assert_eq!(
        vt100_color_to_rgb(vt100::Color::Rgb(0, 0, 255), 0x000),
        0x00F,
        "Max blue RGB"
    );

    // Low nibble edge cases (values < 16 downsample to 0)
    assert_eq!(
        vt100_color_to_rgb(vt100::Color::Rgb(15, 15, 15), 0x000),
        0x000,
        "Values < 16 truncate to 0"
    );
    assert_eq!(
        vt100_color_to_rgb(vt100::Color::Rgb(16, 16, 16), 0x000),
        0x111,
        "Values == 16 truncate to 1"
    );
    assert_eq!(
        vt100_color_to_rgb(vt100::Color::Rgb(240, 240, 240), 0x000),
        0xFFF,
        "Values >= 240 truncate to 15 (0xF)"
    );

    // Verify all 16x16x16 quantized RGB values produce valid 12-bit results
    for r in (0..=255).step_by(17) {
        for g in (0..=255).step_by(17) {
            for b in (0..=255).step_by(17) {
                let res = vt100_color_to_rgb(vt100::Color::Rgb(r as u8, g as u8, b as u8), 0);
                assert!(res <= 0xFFF);
            }
        }
    }
}

// =========================================================================
// 2. Adversarial Tests for extract_text_from_screen
// =========================================================================

#[test]
fn test_extract_text_pathological_selection_ranges() {
    let mut parser = vt100::Parser::new(6, 12, 0);
    parser.process(b"Line 000000\r\nLine 111111\r\nLine 222222\r\nLine 333333");
    let screen = parser.screen();

    // 1. Inverted row: start_row > end_row (backward vertical drag)
    let inverted_row = SelectionRange::new(0, 3, 5, 1);
    let text = extract_text_from_screen(screen, inverted_row);
    assert_eq!(
        text, "",
        "Inverted row range must safely return empty string without panic"
    );

    // 2. Inverted col: start_col > end_col on same row (backward horizontal drag)
    let inverted_col = SelectionRange::new(8, 0, 2, 0);
    let text = extract_text_from_screen(screen, inverted_col);
    assert_eq!(
        text, "",
        "Inverted col on same row must safely return empty string without panic"
    );

    // 3. Inverted col on multi-line selection (start_col > end_col, but start_row < end_row)
    // e.g. from row 0 col 8 to row 1 col 2.
    // Line 0: cols 8..=9 ("00"), newline, Line 1: cols 0..=2 ("Lin")
    let multiline_narrow = SelectionRange::new(9, 0, 2, 1);
    let text = extract_text_from_screen(screen, multiline_narrow);
    assert_eq!(
        text, "00\nLin",
        "Multi-line selection with start_col > end_col must wrap properly"
    );

    // 4. Out of bounds start_row and end_row (beyond screen row count)
    let oob_rows = SelectionRange::new(0, 10, 5, 20);
    let text = extract_text_from_screen(screen, oob_rows);
    assert_eq!(
        text.replace('\n', ""),
        "",
        "Out of bounds rows must return no cell text"
    );

    // 5. Out of bounds cols on valid row
    let oob_cols = SelectionRange::new(50, 0, 100, 0);
    let text = extract_text_from_screen(screen, oob_cols);
    assert_eq!(
        text, "",
        "Out of bounds columns must safely yield empty string"
    );

    // 6. Extreme coordinate values: near u16::MAX
    let max_coords = SelectionRange::new(u16::MAX - 1, u16::MAX - 1, u16::MAX, u16::MAX);
    let text = extract_text_from_screen(screen, max_coords);
    assert_eq!(
        text.replace('\n', ""),
        "",
        "u16::MAX coordinates must not panic or cause arithmetic overflow"
    );

    // 7. Single cell selection at screen boundary (last character '3' at (10, 3))
    let single_cell = SelectionRange::new(10, 3, 10, 3);
    let text = extract_text_from_screen(screen, single_cell);
    assert_eq!(text, "3", "Single cell at bottom-right character");

    // 8. Full screen selection
    let full_screen = SelectionRange::new(0, 0, 10, 3);
    let text = extract_text_from_screen(screen, full_screen);
    assert_eq!(
        text, "Line 000000\nLine 111111\nLine 222222\nLine 333333",
        "Full screen extraction must match full text exactly"
    );
}

#[test]
fn test_extract_text_multibyte_utf8_cjk_arabic_hebrew() {
    let mut parser = vt100::Parser::new(6, 20, 0);
    // Greek, Cyrillic, CJK, and RTL Arabic
    parser.process("αβγδε\r\nПривет мир\r\n日本語テスト\r\nمرحبا".as_bytes());
    let screen = parser.screen();

    // Line 0: Greek
    let greek = extract_text_from_screen(screen, SelectionRange::new(0, 0, 4, 0));
    assert_eq!(greek, "αβγδε");

    // Line 1: Cyrillic
    let cyrillic = extract_text_from_screen(screen, SelectionRange::new(0, 1, 9, 1));
    assert_eq!(cyrillic, "Привет мир");

    // Line 2: CJK wide characters (each occupies 2 columns in terminal)
    // "日本語テスト" is 6 wide characters = 12 columns
    let cjk = extract_text_from_screen(screen, SelectionRange::new(0, 2, 11, 2));
    assert_eq!(cjk, "日本語テスト");

    // Partial selection slicing a wide character in half (only selecting column 1, which is the continuation cell of '日')
    let half_char = extract_text_from_screen(screen, SelectionRange::new(1, 2, 1, 2));
    assert_eq!(
        half_char, "",
        "Selecting only wide continuation cell must yield empty string without panic"
    );

    // Selecting column 0 (the leading cell of '日')
    let lead_char = extract_text_from_screen(screen, SelectionRange::new(0, 2, 0, 2));
    assert_eq!(
        lead_char, "日",
        "Selecting leading wide cell must yield the full glyph"
    );

    // Line 3: Arabic UTF-8
    let arabic = extract_text_from_screen(screen, SelectionRange::new(0, 3, 4, 3));
    assert_eq!(arabic, "مرحبا");
}

#[test]
fn test_extract_text_emoji_zwj_sequences_and_modifiers() {
    let mut parser = vt100::Parser::new(4, 25, 0);
    // 1. Crab emoji (wide, 2 cols)
    // 2. Skin tone modifier: 👍🏽 (thumbs up + medium skin tone)
    // 3. ZWJ sequence: 👨‍👩‍👧‍👦 (family)
    // 4. Rainbow flag: 🏳️‍🌈
    let text_payload = "🦀\n👍🏽\n👨‍👩‍👧‍👦\n🏳️‍🌈";
    parser.process(text_payload.as_bytes());
    let screen = parser.screen();

    // Line 0: Crab emoji
    let crab = extract_text_from_screen(screen, SelectionRange::new(0, 0, 1, 0));
    assert_eq!(crab, "🦀");

    // Line 1: Modified emoji
    let thumb = extract_text_from_screen(screen, SelectionRange::new(0, 1, 3, 1));
    assert!(
        thumb.contains('👍'),
        "Extracted text must contain base emoji"
    );

    // Line 2: ZWJ family sequence
    let family = extract_text_from_screen(screen, SelectionRange::new(0, 2, 8, 2));
    assert!(
        !family.is_empty(),
        "ZWJ sequence must be extractable without panic"
    );

    // Multi-line selection spanning emoji lines
    let multi_emoji = extract_text_from_screen(screen, SelectionRange::new(0, 0, 10, 3));
    assert!(
        multi_emoji.contains("🦀"),
        "Multi-line extraction must preserve emojis"
    );
}

// =========================================================================
// 3. Adversarial Tests for OrpheusRenderer::build_cells
// =========================================================================

#[test]
fn test_build_cells_empty_and_zero_dimension_screens() {
    let parser = vt100::Parser::new(2, 5, 0);
    let screen = parser.screen();

    // 1. Blank screen with default cursor visible
    let mut instances = Vec::new();
    let options = CellBuildOptions::new(2, 5);
    OrpheusRenderer::build_cells(screen, &mut instances, options);
    // Only cursor quad should be emitted because screen has no text and default bg
    assert_eq!(
        instances.len(),
        1,
        "Blank screen should only emit cursor quad"
    );
    assert_eq!(instances[0].grid_position, [0, 0]);

    // 2. Zero rows and zero cols options
    let mut instances_zero = Vec::new();
    let zero_options = CellBuildOptions {
        rows: 0,
        cols: 0,
        cursor_visible: true,
        cursor_style: 0,
    };
    OrpheusRenderer::build_cells(screen, &mut instances_zero, zero_options);
    assert_eq!(
        instances_zero.len(),
        0,
        "Zero dimension options must emit 0 instances"
    );

    // 3. Options larger than screen (screen 2x5, options 100x100)
    let mut instances_large = Vec::new();
    let large_options = CellBuildOptions::new(100, 100);
    OrpheusRenderer::build_cells(screen, &mut instances_large, large_options);
    // Should clamp to screen dimensions without panic and emit cursor quad
    assert_eq!(
        instances_large.len(),
        1,
        "Options larger than screen must clamp gracefully"
    );
}

#[test]
fn test_build_cells_cursor_visibility_and_custom_styles() {
    let mut parser = vt100::Parser::new(2, 5, 0);
    parser.process(b"A");
    let screen = parser.screen();

    // 1. Cursor visible = false
    let mut instances = Vec::new();
    let mut opt_hidden = CellBuildOptions::new(2, 5);
    opt_hidden.cursor_visible = false;
    OrpheusRenderer::build_cells(screen, &mut instances, opt_hidden);
    assert_eq!(
        instances.len(),
        1,
        "Only cell 'A' emitted; cursor quad suppressed"
    );
    assert_eq!(instances[0].grid_position, [0, 0]);

    // 2. Cursor hidden via terminal escape sequence \x1b[?25l
    parser.process(b"\x1b[?25l");
    let mut instances_esc_hidden = Vec::new();
    let opt_default = CellBuildOptions::new(2, 5);
    OrpheusRenderer::build_cells(parser.screen(), &mut instances_esc_hidden, opt_default);
    assert_eq!(
        instances_esc_hidden.len(),
        1,
        "Cursor suppressed by terminal escape state"
    );

    // 3. Unhide cursor (\x1b[?25h) and test cursor style 2 (underline)
    parser.process(b"\x1b[?25h");
    let mut instances_underline = Vec::new();
    let mut opt_underline = CellBuildOptions::new(2, 5);
    opt_underline.cursor_style = 2; // Underline style
    OrpheusRenderer::build_cells(parser.screen(), &mut instances_underline, opt_underline);
    assert_eq!(instances_underline.len(), 2, "Cell 'A' + cursor quad");
    let cursor = &instances_underline[1];
    assert_eq!(cursor.grid_position, [1, 0]); // Cursor advanced past 'A'
    assert_ne!(
        cursor.glyph_id_and_effects & (1 << 13),
        0,
        "Underline cursor must set effect bit 13"
    );

    // 4. Cursor style 1 (beam) or 0 (block)
    let mut instances_block = Vec::new();
    let mut opt_block = CellBuildOptions::new(2, 5);
    opt_block.cursor_style = 0;
    OrpheusRenderer::build_cells(parser.screen(), &mut instances_block, opt_block);
    let cursor_block = &instances_block[1];
    assert_eq!(
        cursor_block.glyph_id_and_effects & (1 << 13),
        0,
        "Block cursor must NOT set underline effect bit"
    );

    // 5. High undefined cursor style (e.g. style 255)
    let mut instances_custom = Vec::new();
    let mut opt_custom = CellBuildOptions::new(2, 5);
    opt_custom.cursor_style = 255;
    OrpheusRenderer::build_cells(parser.screen(), &mut instances_custom, opt_custom);
    assert_eq!(
        instances_custom.len(),
        2,
        "Undefined cursor style must not panic"
    );
}

#[test]
fn test_build_cells_colored_background_spaces_and_underlines() {
    let mut parser = vt100::Parser::new(2, 5, 0);
    // Write colored background spaces: \x1b[44m   \x1b[0m (blue background spaces)
    // Then underlined text: \x1b[4mZ\x1b[0m
    parser.process(b"\x1b[44m  \x1b[0m\x1b[4mZ\x1b[0m");
    let screen = parser.screen();

    let mut instances = Vec::new();
    let mut options = CellBuildOptions::new(2, 5);
    options.cursor_visible = false;
    OrpheusRenderer::build_cells(screen, &mut instances, options);

    // Cell (0,0): space with blue bg -> must emit instance
    // Cell (1,0): space with blue bg -> must emit instance
    // Cell (2,0): 'Z' with underline -> must emit instance with bit 13 set
    // Cell (3,0), (4,0): default empty spaces -> must NOT emit instance
    assert_eq!(
        instances.len(),
        3,
        "Colored background spaces and underlined characters must emit quads"
    );

    assert_eq!(instances[0].grid_position, [0, 0]);
    assert_eq!(instances[1].grid_position, [1, 0]);
    assert_eq!(instances[2].grid_position, [2, 0]);

    // Check underline bit on 'Z'
    assert_ne!(
        instances[2].glyph_id_and_effects & (1 << 13),
        0,
        "Underlined cell must set bit 13"
    );
    // Check no underline bit on spaces
    assert_eq!(
        instances[0].glyph_id_and_effects & (1 << 13),
        0,
        "Un-underlined cell must not set bit 13"
    );
}

// =========================================================================
// 4. Concurrency & Multi-Thread Stress Testing (RingBuffer)
// =========================================================================

#[test]
fn test_ring_buffer_multi_producer_multi_consumer_stress() {
    const CAPACITY: usize = 256;
    const PRODUCER_THREADS: usize = 4;
    const CONSUMER_THREADS: usize = 4;
    const ITEMS_PER_PRODUCER: usize = 10_000;
    const TOTAL_ITEMS: usize = PRODUCER_THREADS * ITEMS_PER_PRODUCER;

    let ring_buffer = RingBuffer::<TypedBlock, CAPACITY>::new();
    let (producer, consumer) = ring_buffer.split();

    let consumed_total = Arc::new(AtomicUsize::new(0));
    let stdout_count = Arc::new(AtomicUsize::new(0));
    let stderr_count = Arc::new(AtomicUsize::new(0));
    let done_producing = Arc::new(AtomicBool::new(false));

    // Spawn consumer threads
    let mut consumer_handles = Vec::new();
    for _ in 0..CONSUMER_THREADS {
        let consumer = consumer.clone();
        let consumed_total = Arc::clone(&consumed_total);
        let stdout_count = Arc::clone(&stdout_count);
        let stderr_count = Arc::clone(&stderr_count);
        let done_producing = Arc::clone(&done_producing);

        consumer_handles.push(thread::spawn(move || {
            while !done_producing.load(Ordering::Acquire)
                || consumed_total.load(Ordering::Acquire) < TOTAL_ITEMS
            {
                if let Some(block) = consumer.try_pop() {
                    match block {
                        TypedBlock::StdoutChunk(_) => {
                            stdout_count.fetch_add(1, Ordering::Relaxed);
                        }
                        TypedBlock::StderrChunk(_) => {
                            stderr_count.fetch_add(1, Ordering::Relaxed);
                        }
                        _ => panic!("Unexpected block type in ring buffer"),
                    }
                    consumed_total.fetch_add(1, Ordering::Release);
                } else {
                    std::hint::spin_loop();
                }
            }
        }));
    }
    // Drop initial consumer handle so drops are tracked properly
    drop(consumer);

    // Spawn producer threads
    let mut producer_handles = Vec::new();
    for p_id in 0..PRODUCER_THREADS {
        let producer = producer.clone();
        producer_handles.push(thread::spawn(move || {
            for i in 0..ITEMS_PER_PRODUCER {
                let block = if (p_id + i) % 2 == 0 {
                    TypedBlock::StdoutChunk(format!("P{p_id}-item-{i}").into_bytes())
                } else {
                    TypedBlock::StderrChunk(format!("P{p_id}-item-{i}").into_bytes())
                };

                while producer.try_push(block.clone()).is_err() {
                    std::hint::spin_loop();
                }
            }
        }));
    }
    // Drop initial producer handle
    drop(producer);

    // Wait for all producers to finish
    for handle in producer_handles {
        handle.join().expect("Producer thread panicked");
    }
    done_producing.store(true, Ordering::Release);

    // Wait for all consumers to finish
    for handle in consumer_handles {
        handle.join().expect("Consumer thread panicked");
    }

    let final_consumed = consumed_total.load(Ordering::SeqCst);
    let final_stdout = stdout_count.load(Ordering::SeqCst);
    let final_stderr = stderr_count.load(Ordering::SeqCst);

    assert_eq!(
        final_consumed, TOTAL_ITEMS,
        "Total items consumed must match total items produced without loss"
    );
    assert_eq!(
        final_stdout + final_stderr,
        TOTAL_ITEMS,
        "Stdout + Stderr counts must equal total items"
    );
}

#[test]
fn test_ring_buffer_tight_capacity_saturation_stress() {
    // Stress test extreme backpressure with minimal capacity buffer (capacity = 2)
    const CAPACITY: usize = 2;
    const ITERATIONS: usize = 25_000;

    let ring_buffer = RingBuffer::<TypedBlock, CAPACITY>::new();
    let (producer, consumer) = ring_buffer.split();

    let handle = thread::spawn(move || {
        for i in 0..ITERATIONS {
            let block = TypedBlock::StdoutChunk(vec![(i % 256) as u8; 64]);
            while producer.try_push(block.clone()).is_err() {
                std::hint::spin_loop();
            }
        }
    });

    let mut received = 0;
    while received < ITERATIONS {
        if let Some(_block) = consumer.try_pop() {
            received += 1;
        } else {
            std::hint::spin_loop();
        }
    }

    handle.join().expect("Producer thread panicked");
    assert_eq!(received, ITERATIONS);
}

#[test]
fn test_ring_buffer_producer_disconnect_detection() {
    let ring_buffer = RingBuffer::<TypedBlock, 8>::new();
    let (producer, consumer) = ring_buffer.split();

    // Push 2 items, then drop producer
    producer
        .try_push(TypedBlock::StdoutChunk(b"item1".to_vec()))
        .unwrap();
    producer
        .try_push(TypedBlock::StdoutChunk(b"item2".to_vec()))
        .unwrap();
    drop(producer);

    // Consumer should see items, then detect closure
    assert!(consumer.try_pop().is_some());
    assert!(consumer.try_pop().is_some());
    assert!(consumer.try_pop().is_none());
    assert!(
        consumer.is_closed(),
        "Consumer must detect when all producers disconnect"
    );
}

#[test]
fn test_ring_buffer_consumer_early_drop_rejection() {
    let ring_buffer = RingBuffer::<TypedBlock, 8>::new();
    let (producer, consumer) = ring_buffer.split();

    // Drop consumer immediately
    drop(consumer);

    assert!(
        producer.is_closed(),
        "Producer must detect disconnected consumer"
    );
    let item = TypedBlock::StdoutChunk(b"dropped".to_vec());
    let err = producer.try_push(item).expect_err("Pushing to closed consumer must fail");
    match err {
        TypedBlock::StdoutChunk(bytes) => assert_eq!(bytes, b"dropped"),
        _ => panic!("Expected returned item"),
    }
}

#[test]
fn test_ring_buffer_heavy_burst_100k_spsc() {
    const CAPACITY: usize = 64;
    const TOTAL: usize = 100_000;

    let ring_buffer = RingBuffer::<TypedBlock, CAPACITY>::new();
    let (producer, consumer) = ring_buffer.split();

    let p_handle = thread::spawn(move || {
        for i in 0..TOTAL {
            let chunk = TypedBlock::StdoutChunk(format!("BURST-{i}").into_bytes());
            while producer.try_push(chunk.clone()).is_err() {
                std::hint::spin_loop();
            }
        }
    });

    let c_handle = thread::spawn(move || {
        let mut count = 0;
        while count < TOTAL {
            if let Some(_block) = consumer.try_pop() {
                count += 1;
            } else {
                std::hint::spin_loop();
            }
        }
        count
    });

    p_handle.join().expect("Producer panicked");
    let received = c_handle.join().expect("Consumer panicked");
    assert_eq!(received, TOTAL);
}

#[test]
fn test_extract_text_and_build_cells_minimal_screen() {
    // 1x1 minimal screen
    let mut parser = vt100::Parser::new(1, 1, 0);
    parser.process(b"W");
    let screen = parser.screen();

    let text = extract_text_from_screen(screen, SelectionRange::new(0, 0, 0, 0));
    assert_eq!(text, "W");

    // When row range is out of bounds, no characters are extracted, but newlines are
    // emitted for each row interval in start_row..end_row.
    let text_oob = extract_text_from_screen(screen, SelectionRange::new(5, 5, 10, 10));
    assert_eq!(text_oob, "\n\n\n\n\n");
    assert_eq!(text_oob.trim(), "");

    // Single OOB row yields completely empty string
    let text_single_oob = extract_text_from_screen(screen, SelectionRange::new(5, 5, 10, 5));
    assert_eq!(text_single_oob, "");

    let mut instances = Vec::new();
    let options = CellBuildOptions::new(1, 1);
    OrpheusRenderer::build_cells(screen, &mut instances, options);
    // Cursor advanced to (0, 1) which is outside options.cols (1), so cursor quad is
    // safely omitted to prevent out-of-bounds rendering. Only 1 cell ('W') is emitted.
    assert_eq!(
        instances.len(),
        1,
        "Only 'W' emitted; cursor past column bound is safely suppressed"
    );
    assert_eq!(instances[0].grid_position, [0, 0]);
}

