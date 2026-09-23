use well_llm::generate_test_pattern_png;

#[test]
fn test_image_load() {
    let png_bytes = generate_test_pattern_png(64, 64, "test");
    let img = image::load_from_memory(&png_bytes);
    assert!(img.is_ok(), "Failed to load image: {:?}", img.err());
}
