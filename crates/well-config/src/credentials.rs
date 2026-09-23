use keyring::{Entry, Error as KeyringError};

pub const KEYCHAIN_SERVICE: &str = "org.well.terminal.ai";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiCredential {
    GeminiApiKey,
    HuggingFaceToken,
}

impl AiCredential {
    pub const fn account(self) -> &'static str {
        match self {
            Self::GeminiApiKey => "gemini-api-key",
            Self::HuggingFaceToken => "hugging-face-token",
        }
    }

    pub const fn environment_variable(self) -> &'static str {
        match self {
            Self::GeminiApiKey => "GEMINI_API_KEY",
            Self::HuggingFaceToken => "HF_TOKEN",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialSource {
    Environment,
    Keychain,
    LegacyConfig,
    Unavailable,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedCredential {
    pub value: Option<String>,
    pub source: CredentialSource,
}

pub trait CredentialStore {
    fn load(&self, credential: AiCredential) -> Result<Option<String>, String>;
    fn save(&self, credential: AiCredential, value: &str) -> Result<(), String>;
    fn delete(&self, credential: AiCredential) -> Result<(), String>;
}

#[derive(Default)]
pub struct OsCredentialStore;

impl OsCredentialStore {
    fn entry(credential: AiCredential) -> Result<Entry, String> {
        Entry::new(KEYCHAIN_SERVICE, credential.account()).map_err(|error| {
            format!(
                "Could not access the OS credential store for {}: {error}",
                credential.account()
            )
        })
    }
}

impl CredentialStore for OsCredentialStore {
    fn load(&self, credential: AiCredential) -> Result<Option<String>, String> {
        match Self::entry(credential)?.get_password() {
            Ok(value) if value.trim().is_empty() => Ok(None),
            Ok(value) => Ok(Some(value)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(format!(
                "Could not read {} from the OS credential store: {error}",
                credential.account()
            )),
        }
    }

    fn save(&self, credential: AiCredential, value: &str) -> Result<(), String> {
        let value = value.trim();
        if value.is_empty() {
            return Err("Refusing to store an empty credential".to_string());
        }
        Self::entry(credential)?
            .set_password(value)
            .map_err(|error| {
                format!(
                    "Could not save {} in the OS credential store: {error}",
                    credential.account()
                )
            })
    }

    fn delete(&self, credential: AiCredential) -> Result<(), String> {
        match Self::entry(credential)?.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(format!(
                "Could not remove {} from the OS credential store: {error}",
                credential.account()
            )),
        }
    }
}

pub fn resolve_credential(
    credential: AiCredential,
    environment_value: Option<String>,
    legacy_value: Option<String>,
    store: &impl CredentialStore,
) -> Result<ResolvedCredential, String> {
    if let Some(value) = environment_value.filter(|value| !value.trim().is_empty()) {
        return Ok(ResolvedCredential {
            value: Some(value),
            source: CredentialSource::Environment,
        });
    }

    if let Some(value) = store.load(credential)? {
        return Ok(ResolvedCredential {
            value: Some(value),
            source: CredentialSource::Keychain,
        });
    }

    if let Some(value) = legacy_value.filter(|value| !value.trim().is_empty()) {
        return Ok(ResolvedCredential {
            value: Some(value),
            source: CredentialSource::LegacyConfig,
        });
    }

    Ok(ResolvedCredential {
        value: None,
        source: CredentialSource::Unavailable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemoryCredentialStore {
        values: Mutex<HashMap<&'static str, String>>,
    }

    impl CredentialStore for MemoryCredentialStore {
        fn load(&self, credential: AiCredential) -> Result<Option<String>, String> {
            Ok(self
                .values
                .lock()
                .unwrap()
                .get(credential.account())
                .cloned())
        }

        fn save(&self, credential: AiCredential, value: &str) -> Result<(), String> {
            self.values
                .lock()
                .unwrap()
                .insert(credential.account(), value.to_string());
            Ok(())
        }

        fn delete(&self, credential: AiCredential) -> Result<(), String> {
            self.values.lock().unwrap().remove(credential.account());
            Ok(())
        }
    }

    #[test]
    fn environment_overrides_keychain_and_legacy_config() {
        let store = MemoryCredentialStore::default();
        store
            .save(AiCredential::GeminiApiKey, "keychain-value")
            .unwrap();

        let resolved = resolve_credential(
            AiCredential::GeminiApiKey,
            Some("environment-value".to_string()),
            Some("legacy-value".to_string()),
            &store,
        )
        .unwrap();

        assert_eq!(resolved.value.as_deref(), Some("environment-value"));
        assert_eq!(resolved.source, CredentialSource::Environment);
    }

    #[test]
    fn keychain_overrides_legacy_config_and_delete_is_idempotent() {
        let store = MemoryCredentialStore::default();
        store
            .save(AiCredential::HuggingFaceToken, "keychain-value")
            .unwrap();

        let resolved = resolve_credential(
            AiCredential::HuggingFaceToken,
            None,
            Some("legacy-value".to_string()),
            &store,
        )
        .unwrap();
        assert_eq!(resolved.value.as_deref(), Some("keychain-value"));
        assert_eq!(resolved.source, CredentialSource::Keychain);

        store.delete(AiCredential::HuggingFaceToken).unwrap();
        store.delete(AiCredential::HuggingFaceToken).unwrap();
        assert_eq!(store.load(AiCredential::HuggingFaceToken).unwrap(), None);
    }

    #[test]
    fn empty_values_are_treated_as_unavailable() {
        let store = MemoryCredentialStore::default();
        let resolved = resolve_credential(
            AiCredential::GeminiApiKey,
            Some("  ".to_string()),
            Some(String::new()),
            &store,
        )
        .unwrap();

        assert_eq!(resolved.value, None);
        assert_eq!(resolved.source, CredentialSource::Unavailable);
        assert_eq!(
            AiCredential::GeminiApiKey.environment_variable(),
            "GEMINI_API_KEY"
        );
        assert_eq!(
            AiCredential::HuggingFaceToken.account(),
            "hugging-face-token"
        );
    }
}
