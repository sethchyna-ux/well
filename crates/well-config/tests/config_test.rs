use std::fs;
use tempfile::tempdir;
use well_config::{capital_color_at, ConfigStore, Preferences, SyntaxTheme};

#[test]
fn test_preferences_default_roundtrip_toml() {
    let prefs = Preferences::default();
    assert!(prefs.validate().is_ok());

    let toml_str = toml::to_string_pretty(&prefs).expect("Serialize to TOML");
    // Verify no secret tokens or credential keys are present in default serialized preferences
    assert!(!toml_str.contains("api_key"));
    assert!(!toml_str.contains("secret"));
    assert!(!toml_str.contains("token"));
    assert!(!toml_str.contains("password"));

    let deserialized: Preferences = toml::from_str(&toml_str).expect("Deserialize from TOML");
    assert_eq!(prefs, deserialized);
}

#[test]
fn test_config_store_atomic_save_and_reload() {
    let dir = tempdir().expect("create temp dir");
    let config_path = dir.path().join("sub").join("settings.toml");

    let (mut store, mut prefs) =
        ConfigStore::load(config_path.clone()).expect("load non-existent returns default");
    assert_eq!(prefs.version, 1);

    prefs.appearance.font_size = 18.0;
    prefs.appearance.line_spacing = 3.5;
    prefs.appearance.font_family = "Menlo".to_string();
    prefs.initial_cols = 120;
    prefs.initial_rows = 40;

    store.save(&prefs).expect("atomic save");
    assert!(config_path.exists());

    // Verify file contents match saved preferences
    let (_store2, loaded_prefs) = ConfigStore::load(config_path).expect("reload saved config");
    assert_eq!(loaded_prefs.appearance.font_size, 18.0);
    assert_eq!(loaded_prefs.appearance.line_spacing, 3.5);
    assert_eq!(loaded_prefs.appearance.font_family, "Menlo");
    assert_eq!(loaded_prefs.initial_cols, 120);
    assert_eq!(loaded_prefs.initial_rows, 40);
}

#[test]
fn test_preferences_validation_boundaries() {
    let mut prefs = Preferences::default();

    // Invalid version
    prefs.version = 2;
    assert!(prefs.validate().is_err());
    prefs.version = 1;

    // Font family with control characters
    prefs.appearance.font_family = "Font\nName".to_string();
    assert!(prefs.validate().is_err());
    prefs.appearance.font_family = "".to_string();
    assert!(prefs.validate().is_err());
    prefs.appearance.font_family = "Monaco".to_string();
    assert!(prefs.validate().is_ok());

    // Font size bounds [8.0, 48.0]
    prefs.appearance.font_size = 7.9;
    assert!(prefs.validate().is_err());
    prefs.appearance.font_size = 48.1;
    assert!(prefs.validate().is_err());
    prefs.appearance.font_size = 14.0;
    assert!(prefs.validate().is_ok());

    // Padding upper bound 64
    prefs.appearance.padding = 65;
    assert!(prefs.validate().is_err());
    prefs.appearance.padding = 16;
    assert!(prefs.validate().is_ok());

    // Initial grid dimensions
    prefs.initial_cols = 19;
    assert!(prefs.validate().is_err());
    prefs.initial_cols = 80;
    prefs.initial_rows = 4;
    assert!(prefs.validate().is_err());
    prefs.initial_rows = 24;
    assert!(prefs.validate().is_ok());
}

#[test]
fn test_syntax_theme_presets() {
    let default_theme = SyntaxTheme::default();
    let cyberpunk = SyntaxTheme::cyberpunk();
    let tokyo = SyntaxTheme::tokyo_neon();
    let monokai = SyntaxTheme::monokai();
    let nord = SyntaxTheme::nord();

    assert_ne!(default_theme.command, cyberpunk.command);
    assert_ne!(cyberpunk.command, tokyo.command);
    assert_ne!(monokai.command, nord.command);

    // Ensure all presets pass TOML serialization roundtrip
    for theme in [&default_theme, &cyberpunk, &tokyo, &monokai, &nord] {
        let serialized = toml::to_string(theme).expect("serialize theme");
        let deserialized: SyntaxTheme = toml::from_str(&serialized).expect("deserialize theme");
        assert_eq!(*theme, deserialized);
    }
}

#[test]
fn test_chromatic_capital_colors() {
    let base = [200, 200, 200];
    // With multicolor disabled, returns base color
    let c1 = capital_color_at('A', 0, base, false);
    assert_eq!(c1, base);

    // With multicolor enabled, deterministically maps char & seed
    let c2 = capital_color_at('A', 0, base, true);
    let c3 = capital_color_at('A', 0, base, true);
    assert_eq!(c2, c3);

    let c4 = capital_color_at('B', 0, base, true);
    // Distinct characters should have deterministic palette colors
    assert_ne!(c2, base);
    assert_ne!(c4, base);
}

#[test]
fn test_config_store_concurrent_modification_detection() {
    let dir = tempdir().expect("create temp dir");
    let config_path = dir.path().join("settings.toml");

    let (mut store, prefs) = ConfigStore::load(config_path.clone()).expect("initial load");
    store.save(&prefs).expect("first save");

    // Simulate external process modifying the file on disk
    fs::write(&config_path, b"# Modified externally\nversion = 1\n").expect("write external");

    // Saving should detect that file changed on disk and return error
    let save_result = store.save(&prefs);
    assert!(save_result.is_err());
    let err_msg = save_result.unwrap_err().to_string();
    assert!(err_msg.contains("Settings changed on disk"));
}
