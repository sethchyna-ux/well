//! Secure OS-backed credential management for AI provider tokens.
//! On macOS, stores credentials in macOS Keychain via Security Framework.
//! On Linux, stores credentials in Secret Service (freedesktop DBus).

const KEYCHAIN_SERVICE: &str = "well_terminal";

/// Fetches a secret from the OS Keychain / Keyring.
pub fn get_secret(key: &str) -> Option<String> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, key).ok()?;
    entry.get_password().ok()
}

/// Stores a secret securely in the OS Keychain / Keyring.
pub fn set_secret(key: &str, secret: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, key).map_err(|e| e.to_string())?;
    entry.set_password(secret).map_err(|e| e.to_string())
}

/// Deletes a secret from the OS Keychain / Keyring.
pub fn delete_secret(key: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, key).map_err(|e| e.to_string())?;
    let _ = entry.delete_credential();
    Ok(())
}

/// Resolves Gemini API key checking in order:
/// 1. Explicit in-memory parameter
/// 2. OS Keychain (`well_terminal/gemini_api_key`)
/// 3. `GEMINI_API_KEY` environment variable
pub fn resolve_gemini_api_key(explicit: Option<&str>) -> Option<String> {
    explicit
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .or_else(|| get_secret("gemini_api_key"))
        .or_else(|| std::env::var("GEMINI_API_KEY").ok())
        .filter(|s| !s.trim().is_empty())
}

/// Resolves Hugging Face token checking in order:
/// 1. Explicit in-memory parameter
/// 2. OS Keychain (`well_terminal/hf_token`)
/// 3. `HF_TOKEN` environment variable
pub fn resolve_hf_token(explicit: Option<&str>) -> Option<String> {
    explicit
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .or_else(|| get_secret("hf_token"))
        .or_else(|| std::env::var("HF_TOKEN").ok())
        .filter(|s| !s.trim().is_empty())
}

/// Resolves OpenAI API key checking in order:
/// 1. Explicit in-memory parameter
/// 2. OS Keychain (`well_terminal/openai_api_key`)
/// 3. `WELL_OPENAI_KEY` or `OPENAI_API_KEY` environment variable
pub fn resolve_openai_key(explicit: Option<&str>) -> Option<String> {
    explicit
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .or_else(|| get_secret("openai_api_key"))
        .or_else(|| std::env::var("WELL_OPENAI_KEY").ok())
        .or_else(|| std::env::var("OPENAI_API_KEY").ok())
        .filter(|s| !s.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_with_explicit() {
        let key = resolve_gemini_api_key(Some("explicit-test-key"));
        assert_eq!(key.as_deref(), Some("explicit-test-key"));

        let token = resolve_hf_token(Some("explicit-hf-token"));
        assert_eq!(token.as_deref(), Some("explicit-hf-token"));

        let openai_key = resolve_openai_key(Some("  sk-explicit-openai-key  "));
        assert_eq!(openai_key.as_deref(), Some("sk-explicit-openai-key"));
    }

    #[test]
    fn test_resolve_openai_key_from_env() {
        std::env::set_var("WELL_OPENAI_KEY", "sk-well-env-key-123");
        let resolved = resolve_openai_key(None);
        // Either keychain (if set on host) or env var is returned
        assert!(resolved.is_some());
        if get_secret("openai_api_key").is_none() {
            assert_eq!(resolved.as_deref(), Some("sk-well-env-key-123"));
        }
        std::env::remove_var("WELL_OPENAI_KEY");
    }
}
