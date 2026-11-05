use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const DIAGNOSTIC_SCHEMA_VERSION: u32 = 1;

pub fn startup_command_output(args: &[String]) -> Result<Option<String>, String> {
    let filtered: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|argument| !argument.starts_with("-psn_"))
        .collect();

    match filtered.as_slice() {
        [] => Ok(None),
        ["--version" | "-V"] => Ok(Some(format!("Well {}", env!("CARGO_PKG_VERSION")))),
        ["--diagnose"] => serde_json::to_string_pretty(&diagnostic_report())
            .map(Some)
            .map_err(|error| format!("Could not serialize diagnostics: {error}")),
        ["--help" | "-h"] => Ok(Some(help_text().to_string())),
        ["--diagnose", ..] => Err("--diagnose does not accept additional arguments".to_string()),
        _ => Ok(None),
    }
}

fn help_text() -> &'static str {
    "Well Terminal\n\nUsage: Well [--version | --diagnose | --help]\n\n  --version   Print the application version without launching the UI.\n  --diagnose  Print a privacy-redacted JSON health report without launching the UI.\n  --help      Print this help without launching the UI."
}

pub fn diagnostic_report() -> Value {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let executable = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("Well"));
    let config_path = well_config::ConfigStore::default_path().ok();
    let shell = std::env::var_os("SHELL").map(PathBuf::from);

    build_report(
        home.as_deref(),
        &executable,
        config_path.as_deref(),
        shell.as_deref(),
        environment_credential_present("GEMINI_API_KEY"),
        environment_credential_present("HF_TOKEN"),
    )
}

fn build_report(
    home: Option<&Path>,
    executable: &Path,
    config_path: Option<&Path>,
    shell: Option<&Path>,
    gemini_environment_credential: bool,
    hugging_face_environment_credential: bool,
) -> Value {
    let config = inspect_config(config_path, home);
    let executable_name = executable
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Well");
    let is_macos_bundle = executable
        .to_string_lossy()
        .contains(".app/Contents/MacOS/");
    let generated_images = home
        .map(|path| path.join(".well").join("generated"))
        .unwrap_or_else(|| PathBuf::from("~/.well/generated"));

    json!({
        "schema_version": DIAGNOSTIC_SCHEMA_VERSION,
        "application": {
            "name": "Well",
            "version": env!("CARGO_PKG_VERSION"),
            "target_os": std::env::consts::OS,
            "target_arch": std::env::consts::ARCH,
            "executable": executable_name,
            "macos_app_bundle": is_macos_bundle,
        },
        "configuration": config,
        "runtime": {
            "shell_configured": shell.is_some(),
            "shell_available": shell.is_some_and(Path::is_file),
            "generated_images": redact_home(&generated_images, home),
        },
        "credentials": {
            "gemini_environment_present": gemini_environment_credential,
            "hugging_face_environment_present": hugging_face_environment_credential,
            "os_credential_store": "not inspected",
            "values_included": false,
        },
        "privacy": {
            "terminal_output_included": false,
            "command_history_included": false,
            "environment_values_included": false,
            "credential_values_included": false,
        }
    })
}

fn inspect_config(config_path: Option<&Path>, home: Option<&Path>) -> Value {
    let Some(path) = config_path else {
        return json!({
            "path": null,
            "status": "home unavailable",
            "legacy_credentials_present": false,
        });
    };

    let display_path = redact_home(path, home);
    if !path.exists() {
        return json!({
            "path": display_path,
            "status": "missing (defaults active)",
            "legacy_credentials_present": false,
        });
    }

    let data = match std::fs::read_to_string(path) {
        Ok(data) => data,
        Err(_) => {
            return json!({
                "path": display_path,
                "status": "unreadable",
                "legacy_credentials_present": false,
            });
        }
    };
    let legacy_credentials_present = serde_json::from_str::<Value>(&data)
        .map(|value| contains_non_empty_credential(&value))
        .unwrap_or(false);

    if toml::from_str::<well_config::Preferences>(&data).is_ok()
        || serde_json::from_str::<Value>(&data).is_ok()
    {
        json!({
            "path": display_path,
            "status": "valid",
            "schema_version": 2,
            "legacy_credentials_present": legacy_credentials_present,
        })
    } else {
        json!({
            "path": display_path,
            "status": "invalid",
            "legacy_credentials_present": legacy_credentials_present,
        })
    }
}

fn contains_non_empty_credential(value: &Value) -> bool {
    match value {
        Value::Object(values) => values.iter().any(|(key, value)| {
            (matches!(key.as_str(), "gemini_api_key" | "hf_token")
                && value
                    .as_str()
                    .is_some_and(|secret| !secret.trim().is_empty()))
                || contains_non_empty_credential(value)
        }),
        Value::Array(values) => values.iter().any(contains_non_empty_credential),
        _ => false,
    }
}

fn environment_credential_present(name: &str) -> bool {
    std::env::var(name)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn redact_home(path: &Path, home: Option<&Path>) -> String {
    if let Some(home) = home {
        if let Ok(relative) = path.strip_prefix(home) {
            if relative.as_os_str().is_empty() {
                return "$HOME".to_string();
            }
            return format!("$HOME/{}", relative.to_string_lossy());
        }
    }
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostics_version_flag() {
        let args = vec!["--version".to_string()];
        let out = startup_command_output(&args).unwrap();
        assert!(out.is_some());
        assert!(out.unwrap().starts_with("Well "));
    }

    #[test]
    fn test_diagnostics_help_flag() {
        let args = vec!["--help".to_string()];
        let out = startup_command_output(&args).unwrap();
        assert!(out.is_some());
        assert!(out.unwrap().contains("Usage: Well"));
    }

    #[test]
    fn test_diagnostics_no_flag_returns_none() {
        let args: Vec<String> = vec![];
        let out = startup_command_output(&args).unwrap();
        assert!(out.is_none());
    }

    #[test]
    fn test_diagnostics_home_redaction() {
        let home = Path::new("/Users/testuser");
        let subpath = Path::new("/Users/testuser/Library/Application Support/Well/settings.toml");
        let redacted = redact_home(subpath, Some(home));
        assert_eq!(
            redacted,
            "$HOME/Library/Application Support/Well/settings.toml"
        );
        assert!(!redacted.contains("testuser"));

        let exact_home = Path::new("/Users/testuser");
        assert_eq!(redact_home(exact_home, Some(home)), "$HOME");
    }
}
