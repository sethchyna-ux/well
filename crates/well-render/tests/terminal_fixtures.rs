use well_render::{
    extract_text_from_screen, vt100_color_to_rgb, CellBuildOptions, OrpheusRenderer, SelectionRange,
};

fn cell(screen: &vt100::Screen, row: u16, col: u16) -> &vt100::Cell {
    screen
        .cell(row, col)
        .unwrap_or_else(|| panic!("missing terminal cell at ({row}, {col})"))
}

#[test]
fn ansi_indexed_and_rgb_colors_reach_render_instances() {
    let mut parser = vt100::Parser::new(2, 8, 0);
    parser.process(b"\x1b[31mA\x1b[38;5;196mB\x1b[38;2;18;171;240;48;2;240;32;16mC\x1b[0m");

    let screen = parser.screen();
    assert_eq!(cell(screen, 0, 0).fgcolor(), vt100::Color::Idx(1));
    assert_eq!(cell(screen, 0, 1).fgcolor(), vt100::Color::Idx(196));
    assert_eq!(
        cell(screen, 0, 2).fgcolor(),
        vt100::Color::Rgb(18, 171, 240)
    );
    assert_eq!(cell(screen, 0, 2).bgcolor(), vt100::Color::Rgb(240, 32, 16));

    assert_eq!(vt100_color_to_rgb(vt100::Color::Idx(1), 0x123), 0xF08);
    assert_eq!(vt100_color_to_rgb(vt100::Color::Idx(196), 0x123), 0xF00);
    assert_eq!(
        vt100_color_to_rgb(vt100::Color::Rgb(18, 171, 240), 0x123),
        0x1AF
    );

    let mut instances = Vec::new();
    let mut options = CellBuildOptions::new(2, 8);
    options.cursor_visible = false;
    OrpheusRenderer::build_cells(screen, &mut instances, options);

    assert_eq!(instances.len(), 3);
    assert_eq!(instances[0].grid_position, [0, 0]);
    assert_eq!(instances[0].packed_colors, 0xF08 << 12);
    assert_eq!(instances[1].grid_position, [1, 0]);
    assert_eq!(instances[1].packed_colors, 0xF00 << 12);
    assert_eq!(instances[2].grid_position, [2, 0]);
    assert_eq!(instances[2].packed_colors, (0x1AF << 12) | 0xF21);
}

#[test]
fn cursor_visibility_escape_and_renderer_cursor_styles_are_respected() {
    let mut parser = vt100::Parser::new(2, 8, 0);
    parser.process(b"x\x1b[?25l");

    assert!(parser.screen().hide_cursor());
    assert_eq!(parser.screen().cursor_position(), (0, 1));

    let mut instances = Vec::new();
    OrpheusRenderer::build_cells(parser.screen(), &mut instances, CellBuildOptions::new(2, 8));
    assert_eq!(instances.len(), 1, "hidden cursor must not emit a quad");

    parser.process(b"\x1b[?25h\x1b[5 q");
    assert!(!parser.screen().hide_cursor());
    assert_eq!(parser.screen().contents(), "x");
    assert_eq!(parser.screen().cursor_position(), (0, 1));

    // vt100 exposes DECTCEM visibility but not the DECSCUSR shape. Verify the
    // style sequence is consumed above, then exercise Well's exposed styles.
    let mut beam_options = CellBuildOptions::new(2, 8);
    beam_options.cursor_style = 1;
    OrpheusRenderer::build_cells(parser.screen(), &mut instances, beam_options);
    let beam = instances.last().expect("beam cursor instance");
    assert_eq!(beam.grid_position, [1, 0]);
    assert_eq!(beam.glyph_id_and_effects & (1 << 13), 0);

    let mut underline_options = CellBuildOptions::new(2, 8);
    underline_options.cursor_style = 2;
    OrpheusRenderer::build_cells(parser.screen(), &mut instances, underline_options);
    let underline = instances.last().expect("underline cursor instance");
    assert_eq!(underline.grid_position, [1, 0]);
    assert_ne!(underline.glyph_id_and_effects & (1 << 13), 0);
}

#[test]
fn text_wraps_at_the_right_margin_without_a_logical_newline() {
    let mut parser = vt100::Parser::new(3, 5, 0);
    parser.process(b"abcdef");

    let screen = parser.screen();
    assert!(screen.row_wrapped(0));
    assert!(!screen.row_wrapped(1));
    assert_eq!(screen.contents(), "abcdef");
    assert_eq!(cell(screen, 0, 4).contents(), "e");
    assert_eq!(cell(screen, 1, 0).contents(), "f");
    assert_eq!(screen.cursor_position(), (1, 1));
    assert_eq!(
        extract_text_from_screen(screen, SelectionRange::new(0, 0, 0, 1)),
        "abcde\nf"
    );
}

#[test]
fn wide_unicode_occupies_a_leading_and_continuation_cell() {
    let mut parser = vt100::Parser::new(2, 8, 0);
    parser.process("A界B".as_bytes());

    let screen = parser.screen();
    assert_eq!(cell(screen, 0, 0).contents(), "A");
    assert_eq!(cell(screen, 0, 1).contents(), "界");
    assert!(cell(screen, 0, 1).is_wide());
    assert_eq!(cell(screen, 0, 2).contents(), "");
    assert!(cell(screen, 0, 2).is_wide_continuation());
    assert_eq!(cell(screen, 0, 3).contents(), "B");
    assert_eq!(screen.cursor_position(), (0, 4));
}

#[test]
fn combining_character_remains_in_the_base_character_cell() {
    let mut parser = vt100::Parser::new(2, 8, 0);
    parser.process("e\u{301}x".as_bytes());

    let screen = parser.screen();
    assert_eq!(cell(screen, 0, 0).contents(), "e\u{301}");
    assert!(!cell(screen, 0, 0).is_wide());
    assert_eq!(cell(screen, 0, 1).contents(), "x");
    assert_eq!(screen.cursor_position(), (0, 2));
    assert_eq!(
        extract_text_from_screen(screen, SelectionRange::new(0, 0, 1, 0)),
        "e\u{301}x"
    );
}

#[test]
fn alternate_screen_exit_restores_primary_contents_and_cursor() {
    let mut parser = vt100::Parser::new(3, 12, 8);
    parser.process(b"primary");
    assert_eq!(parser.screen().cursor_position(), (0, 7));

    parser.process(b"\x1b[?1049h");
    assert!(parser.screen().alternate_screen());
    assert_eq!(parser.screen().contents(), "");

    parser.process(b"alternate");
    assert_eq!(parser.screen().contents(), "alternate");
    assert_eq!(parser.screen().cursor_position(), (0, 9));

    parser.process(b"\x1b[?1049l");
    assert!(!parser.screen().alternate_screen());
    assert_eq!(parser.screen().contents(), "primary");
    assert_eq!(parser.screen().cursor_position(), (0, 7));

    parser.process(b"!");
    assert_eq!(parser.screen().contents(), "primary!");
    assert_eq!(parser.screen().cursor_position(), (0, 8));
}
