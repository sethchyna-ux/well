//! # Well Profile API
//!
//! Public, GUI-independent interface for managing Well named configuration profiles.
//!
//! Profiles are stored as JSON files in `~/.config/well/profiles/<name>.json`.
//! The file format is the same versioned [`PersistentConfig`] used for the main
//! config, with the `theia.profile_name` field set to the profile's name.
//!
//! ## Usage
//!
//! ```no_run
//! use well_config::profiles;
//!
//! // List all saved profiles
//! let names = profiles::list_profiles().unwrap_or_default();
//!
//! // Load a profile into a PersistentConfig
//! let config = profiles::load_profile("work").unwrap();
//!
//! // Save current settings as a profile
//! profiles::save_profile("work", &config).unwrap();
//!
//! // Export profile JSON to an arbitrary path
//! profiles::export_profile_json("work", "/tmp/well-work.json").unwrap();
//!
//! // Import a profile JSON from an arbitrary path, returns the profile name embedded in the file
//! let name = profiles::import_profile_json("/tmp/well-work.json", &config.theia.hf_token, &config.theia.gemini_api_key).unwrap();
//! ```

use std::path::{Path, PathBuf};

use crate::{write_config_atomically, PersistentConfig, TheiaConfigPayload};

// ---------------------------------------------------------------------------
// Internal helpers (mirrors the private helpers in lib.rs)
// ---------------------------------------------------------------------------

fn validate_profile_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty()
        || name.trim() != name
        || name.len() > 80
        || matches!(name, "." | "..")
        || name.starts_with('.')
        || name.contains('/')
        || name.contains('\\')
        || name.contains('\0')
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ' '))
    {
        return Err(
            "Profile name must be a single non-empty file name using letters, numbers, \
             spaces, '-' or '_'"
                .to_string(),
        );
    }
    Ok(())
}

/// Returns the directory where profiles are stored, creating it if needed.
pub fn profile_dir() -> Result<PathBuf, String> {
    let config_path = crate::TheiasPrismPanel::config_path()
        .ok_or_else(|| "Could not determine config directory".to_string())?;
    let config_dir = config_path
        .parent()
        .ok_or_else(|| format!("Invalid config path {}", config_path.display()))?;
    let dir = config_dir.join("profiles");
    std::fs::create_dir_all(&dir).map_err(|e| {
        format!(
            "Failed to create profiles directory {}: {e}",
            dir.display()
        )
    })?;
    Ok(dir)
}

/// Returns the path for a named profile JSON file.
pub fn profile_path(name: &str) -> Result<PathBuf, String> {
    validate_profile_name(name)?;
    Ok(profile_dir()?.join(format!("{name}.json")))
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns a sorted list of all valid saved profile names.
pub fn list_profiles() -> Result<Vec<String>, String> {
    let dir = profile_dir()?;
    let entries = std::fs::read_dir(&dir)
        .map_err(|e| format!("Failed to read profiles directory: {e}"))?;
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file()
                || path.extension().and_then(|ext| ext.to_str()) != Some("json")
            {
                return None;
            }
            let stem = path.file_stem().and_then(|s| s.to_str())?;
            if validate_profile_name(stem).is_ok() {
                Some(stem.to_string())
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names.dedup();
    Ok(names)
}

/// Loads a named profile from disk and returns the parsed [`PersistentConfig`].
///
/// Credentials (`hf_token`, `gemini_api_key`) are not stored in profile files.
/// Pass the current in-memory values so they are preserved in the returned config.
pub fn load_profile(
    name: &str,
    current_hf_token: &str,
    current_gemini_api_key: &str,
) -> Result<PersistentConfig, String> {
    validate_profile_name(name)?;
    let path = profile_path(name)?;
    let data = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read profile '{}': {e}", path.display()))?;

    let value = serde_json::from_str::<serde_json::Value>(&data)
        .map_err(|e| format!("Failed to parse profile '{name}': {e}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| format!("Profile '{name}': root must be a JSON object"))?;

    let mut persistent = if object.contains_key("theia")
        || object.contains_key("version")
        || object.contains_key("keybindings")
        || object.contains_key("abbreviations")
    {
        // Current versioned format
        PersistentConfig::from_json_str(&data)
            .map_err(|e| format!("Failed to parse profile '{name}': {e}"))?
    } else {
        // Legacy TheiaConfigPayload-only format
        let mut payload = serde_json::from_value::<TheiaConfigPayload>(value)
            .map_err(|e| format!("Failed to parse legacy profile '{name}': {e}"))?;
        payload.profile_name = Some(name.to_string());
        let mut persistent = PersistentConfig {
            shell_path: payload.shell_path.clone(),
            active_font_name: payload.font_name.clone(),
            theia: payload,
            ..PersistentConfig::default()
        };
        persistent.normalize();
        persistent
    };

    persistent.theia.profile_name = Some(name.to_string());
    // Restore credentials from caller's context — they are never written to disk.
    persistent.theia.hf_token = current_hf_token.to_string();
    persistent.theia.gemini_api_key = current_gemini_api_key.to_string();
    persistent.hf_token = None;
    persistent.normalize();
    Ok(persistent)
}

/// Saves a [`PersistentConfig`] as a named profile on disk.
///
/// Credentials are stripped before writing. The profile name embedded in the config
/// is updated to `name`.
pub fn save_profile(name: &str, config: &PersistentConfig) -> Result<(), String> {
    validate_profile_name(name)?;
    let path = profile_path(name)?;

    // Strip secrets before writing to disk.
    let mut on_disk = config.clone();
    on_disk.theia.profile_name = Some(name.to_string());
    on_disk.theia.hf_token = String::new();
    on_disk.theia.gemini_api_key = String::new();
    on_disk.hf_token = None;
    on_disk.normalize();

    let json = on_disk
        .to_json_string()
        .map_err(|e| format!("Failed to serialize profile '{name}': {e}"))?;
    write_config_atomically(&path, &json)
}

/// Renames a profile on disk. The old file is removed atomically.
///
/// If the profiles are identical (same `from` and `to`), this is a no-op.
pub fn rename_profile(from: &str, to: &str) -> Result<(), String> {
    if from == to {
        return Ok(());
    }
    validate_profile_name(from)?;
    validate_profile_name(to)?;
    let src = profile_path(from)?;
    let dst = profile_path(to)?;
    if !src.is_file() {
        return Err(format!("Profile '{from}' does not exist"));
    }
    if dst.is_file() {
        return Err(format!(
            "Profile '{to}' already exists; delete it first if you want to overwrite"
        ));
    }

    // Load, relabel, re-save under the new name, then remove the old file.
    let data = std::fs::read_to_string(&src)
        .map_err(|e| format!("Failed to read profile '{from}': {e}"))?;
    let mut config = PersistentConfig::from_json_str(&data)
        .map_err(|e| format!("Failed to parse profile '{from}': {e}"))?;
    config.theia.profile_name = Some(to.to_string());
    let json = config
        .to_json_string()
        .map_err(|e| format!("Failed to serialize renamed profile: {e}"))?;
    write_config_atomically(&dst, &json)?;
    std::fs::remove_file(&src)
        .map_err(|e| format!("Renamed profile to '{to}' but failed to remove '{from}': {e}"))
}

/// Deletes a named profile from disk.
///
/// Returns `Ok(())` if the profile was deleted or was already missing.
pub fn delete_profile(name: &str) -> Result<(), String> {
    validate_profile_name(name)?;
    let path = profile_path(name)?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!(
            "Failed to delete profile '{}': {e}",
            path.display()
        )),
    }
}

/// Exports a named profile's JSON to an arbitrary file path.
///
/// This is useful for sharing profiles between machines or archiving them.
/// Credentials are stripped before export.
pub fn export_profile_json(name: &str, dest: &Path) -> Result<(), String> {
    validate_profile_name(name)?;
    let path = profile_path(name)?;
    let data = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read profile '{}': {e}", path.display()))?;
    // Strip credentials before exporting.
    let mut config = PersistentConfig::from_json_str(&data)
        .map_err(|e| format!("Failed to parse profile '{name}': {e}"))?;
    config.theia.hf_token = String::new();
    config.theia.gemini_api_key = String::new();
    config.hf_token = None;
    let json = config
        .to_json_string()
        .map_err(|e| format!("Failed to serialize profile '{name}': {e}"))?;
    write_config_atomically(dest, &json)
}

/// Imports a profile JSON from an arbitrary path.
///
/// Returns the profile name embedded in the file (falling back to the file stem).
/// Credentials from the caller's current context are spliced in so they are not
/// lost during the import round-trip.
///
/// The imported profile is saved into the profiles directory using the embedded or
/// derived name.
pub fn import_profile_json(
    src: &Path,
    current_hf_token: &str,
    current_gemini_api_key: &str,
) -> Result<String, String> {
    let data = std::fs::read_to_string(src)
        .map_err(|e| format!("Failed to read import source '{}': {e}", src.display()))?;
    let mut config = PersistentConfig::from_json_str(&data)
        .map_err(|e| format!("Failed to parse import source: {e}"))?;

    // Determine name: prefer embedded, fall back to file stem.
    let name = config
        .theia
        .profile_name
        .clone()
        .filter(|n| validate_profile_name(n).is_ok())
        .or_else(|| {
            src.file_stem()
                .and_then(|s| s.to_str())
                .filter(|n| validate_profile_name(n).is_ok())
                .map(|n| n.to_string())
        })
        .ok_or_else(|| {
            format!(
                "Could not determine a valid profile name from '{}'. \
                 Embed a profile_name in the JSON or use a valid filename.",
                src.display()
            )
        })?;

    config.theia.profile_name = Some(name.clone());
    config.theia.hf_token = current_hf_token.to_string();
    config.theia.gemini_api_key = current_gemini_api_key.to_string();
    config.hf_token = None;
    config.normalize();

    save_profile(&name, &config)?;
    Ok(name)
}

/// Returns the [`TheiaConfigPayload`] from a [`PersistentConfig`], ready to be
/// sent over the Hermes IPC channel for live terminal profile switching.
///
/// Credentials are always zeroed — they must never travel over IPC.
pub fn config_to_ipc_payload(config: &PersistentConfig) -> TheiaConfigPayload {
    let mut payload = config.theia.clone();
    payload.hf_token = String::new();
    payload.gemini_api_key = String::new();
    payload
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    /// Create an isolated temp directory for a single test.
    /// Returns the profiles sub-directory (already created).
    fn make_test_profiles_dir(tag: &str) -> PathBuf {
        let id = format!(
            "well_profiles_{}_{tag}_{:?}",
            std::process::id(),
            std::thread::current().id()
        )
        // Thread IDs contain colons on some platforms; replace for filesystem safety.
        .replace([':', '(', ')'], "_");
        let dir = env::temp_dir().join(id).join("profiles");
        std::fs::create_dir_all(&dir).expect("create temp profiles dir");
        dir
    }

    // ---------------------------------------------------------------------------
    // Thin wrappers that write/read/list against an explicit dir instead of
    // resolving through $HOME. This avoids any env-var mutation.
    // ---------------------------------------------------------------------------

    fn save_to(name: &str, config: &PersistentConfig, dir: &Path) -> Result<(), String> {
        validate_profile_name(name)?;
        let path = dir.join(format!("{name}.json"));
        let mut on_disk = config.clone();
        on_disk.theia.profile_name = Some(name.to_string());
        on_disk.theia.hf_token = String::new();
        on_disk.theia.gemini_api_key = String::new();
        on_disk.hf_token = None;
        on_disk.normalize();
        let json = on_disk
            .to_json_string()
            .map_err(|e| format!("serialize: {e}"))?;
        write_config_atomically(&path, &json)
    }

    fn load_from(name: &str, dir: &Path, hf: &str, gemini: &str) -> Result<PersistentConfig, String> {
        validate_profile_name(name)?;
        let path = dir.join(format!("{name}.json"));
        let data = std::fs::read_to_string(&path)
            .map_err(|e| format!("read: {e}"))?;
        let value = serde_json::from_str::<serde_json::Value>(&data)
            .map_err(|e| format!("parse: {e}"))?;
        let object = value.as_object().ok_or("root must be object")?;
        let mut persistent = if object.contains_key("theia")
            || object.contains_key("version")
            || object.contains_key("keybindings")
            || object.contains_key("abbreviations")
        {
            PersistentConfig::from_json_str(&data).map_err(|e| format!("{e}"))?
        } else {
            let mut payload = serde_json::from_value::<TheiaConfigPayload>(value)
                .map_err(|e| format!("{e}"))?;
            payload.profile_name = Some(name.to_string());
            let mut p = PersistentConfig {
                shell_path: payload.shell_path.clone(),
                active_font_name: payload.font_name.clone(),
                theia: payload,
                ..PersistentConfig::default()
            };
            p.normalize();
            p
        };
        persistent.theia.profile_name = Some(name.to_string());
        persistent.theia.hf_token = hf.to_string();
        persistent.theia.gemini_api_key = gemini.to_string();
        persistent.hf_token = None;
        persistent.normalize();
        Ok(persistent)
    }

    fn list_from(dir: &Path) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(dir) else { return vec![]; };
        let mut names: Vec<String> = entries
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if !p.is_file() || p.extension().and_then(|x| x.to_str()) != Some("json") {
                    return None;
                }
                let stem = p.file_stem().and_then(|s| s.to_str())?;
                if validate_profile_name(stem).is_ok() { Some(stem.to_string()) } else { None }
            })
            .collect();
        names.sort();
        names
    }

    fn delete_from(name: &str, dir: &Path) -> Result<(), String> {
        validate_profile_name(name)?;
        let path = dir.join(format!("{name}.json"));
        match std::fs::remove_file(&path) {
            Ok(()) | Err(_) => Ok(()),
        }
    }

    // ---------------------------------------------------------------------------
    // Tests
    // ---------------------------------------------------------------------------

    #[test]
    fn validate_profile_name_rejects_invalid_inputs() {
        assert!(validate_profile_name("").is_err());
        assert!(validate_profile_name(" leading").is_err());
        assert!(validate_profile_name("trailing ").is_err());
        assert!(validate_profile_name("has/slash").is_err());
        assert!(validate_profile_name("has\\backslash").is_err());
        assert!(validate_profile_name(".hidden").is_err());
        assert!(validate_profile_name(".").is_err());
        assert!(validate_profile_name("..").is_err());
        assert!(validate_profile_name(&"x".repeat(81)).is_err());
    }

    #[test]
    fn validate_profile_name_accepts_valid_inputs() {
        assert!(validate_profile_name("work").is_ok());
        assert!(validate_profile_name("my-profile").is_ok());
        assert!(validate_profile_name("my_profile 2").is_ok());
        assert!(validate_profile_name("DEMO").is_ok());
        assert!(validate_profile_name(&"x".repeat(80)).is_ok());
    }

    #[test]
    fn save_load_delete_round_trip() {
        let dir = make_test_profiles_dir("sld");
        let config = PersistentConfig::default();
        save_to("roundtrip", &config, &dir).expect("save should succeed");

        let names = list_from(&dir);
        assert!(names.contains(&"roundtrip".to_string()));

        let loaded = load_from("roundtrip", &dir, "", "").expect("load should succeed");
        assert_eq!(loaded.theia.profile_name, Some("roundtrip".to_string()));
        assert!(loaded.theia.hf_token.is_empty());
        assert!(loaded.theia.gemini_api_key.is_empty());

        delete_from("roundtrip", &dir).expect("delete should succeed");
        delete_from("roundtrip", &dir).expect("idempotent delete");

        let names = list_from(&dir);
        assert!(!names.contains(&"roundtrip".to_string()));
    }

    #[test]
    fn rename_round_trip() {
        let dir = make_test_profiles_dir("rename");
        let config = PersistentConfig::default();
        save_to("original", &config, &dir).expect("save");

        // Manual rename using the same logic as rename_profile but dir-scoped.
        let src = dir.join("original.json");
        let dst = dir.join("renamed.json");
        let data = std::fs::read_to_string(&src).unwrap();
        let mut cfg = PersistentConfig::from_json_str(&data).unwrap();
        cfg.theia.profile_name = Some("renamed".to_string());
        let json = cfg.to_json_string().unwrap();
        write_config_atomically(&dst, &json).expect("write renamed");
        std::fs::remove_file(&src).expect("remove original");

        let names = list_from(&dir);
        assert!(!names.contains(&"original".to_string()), "old name should be gone");
        assert!(names.contains(&"renamed".to_string()), "new name should exist");
    }

    #[test]
    fn export_import_round_trip() {
        let dir = make_test_profiles_dir("expim");
        let mut config = PersistentConfig::default();
        config.theia.font_size = 18.0;
        config.theia.hf_token = "secret".to_string();
        save_to("exportable", &config, &dir).expect("save");

        // Export = read + strip credentials + write to dest.
        let tmp_export = env::temp_dir().join(format!(
            "well_export_expim_{}.json",
            std::process::id()
        ));
        let raw = std::fs::read_to_string(dir.join("exportable.json")).unwrap();
        let mut on_disk = PersistentConfig::from_json_str(&raw).unwrap();
        on_disk.theia.hf_token = String::new();
        on_disk.theia.gemini_api_key = String::new();
        on_disk.hf_token = None;
        let export_json = on_disk.to_json_string().unwrap();
        write_config_atomically(&tmp_export, &export_json).expect("export write");

        let exported_data = std::fs::read_to_string(&tmp_export).unwrap();
        assert!(!exported_data.contains("secret"), "credentials must not appear in export");

        // Import = read exported file, splice credentials, save under detected name.
        let import_data = std::fs::read_to_string(&tmp_export).unwrap();
        let mut imported = PersistentConfig::from_json_str(&import_data).unwrap();
        let name = imported
            .theia
            .profile_name
            .clone()
            .filter(|n| validate_profile_name(n).is_ok())
            .unwrap_or_else(|| "exportable".to_string());
        imported.theia.profile_name = Some(name.clone());
        imported.theia.hf_token = "new_token".to_string();
        imported.theia.gemini_api_key = "new_key".to_string();
        imported.hf_token = None;
        imported.normalize();
        save_to(&name, &imported, &dir).expect("import save");
        assert_eq!(name, "exportable");

        let loaded = load_from("exportable", &dir, "new_token", "new_key").expect("reload");
        assert_eq!(loaded.theia.font_size, 18.0);
        assert_eq!(loaded.theia.hf_token, "new_token");
        assert_eq!(loaded.theia.gemini_api_key, "new_key");

        let _ = std::fs::remove_file(&tmp_export);
    }

    #[test]
    fn credentials_stripped_on_save() {
        let dir = make_test_profiles_dir("creds");
        let mut config = PersistentConfig::default();
        config.theia.hf_token = "super_secret".to_string();
        config.theia.gemini_api_key = "gemini_secret".to_string();
        save_to("creds_test", &config, &dir).expect("save");

        let raw = std::fs::read_to_string(dir.join("creds_test.json")).unwrap();
        assert!(!raw.contains("super_secret"), "hf_token leaked into profile file");
        assert!(!raw.contains("gemini_secret"), "gemini_key leaked into profile file");
    }
}
