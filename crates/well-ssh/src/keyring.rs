use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum KeyringError {
    #[error("Hardware enclave authentication failed: {0}")]
    AuthFailed(String),
    #[error("Key not found in hardware storage")]
    KeyNotFound,
    #[error("Operation cancelled by user")]
    UserCancelled,
    #[error("Hardware signing failure: {0}")]
    SigningError(String),
}

#[derive(Clone, Debug)]
pub struct EnclaveKeyInfo {
    pub key_id: String,
    pub algorithm: String, // e.g., "ssh-ed25519" or "ecdsa-sha2-nistp256"
    pub public_key: Vec<u8>,
    pub requires_touch: bool,
}

#[async_trait]
pub trait KeyringProvider: Send + Sync {
    async fn list_keys(&self) -> Result<Vec<EnclaveKeyInfo>, KeyringError>;
    async fn sign(&self, key_id: &str, data: &[u8]) -> Result<Vec<u8>, KeyringError>;
}

// ---------------------------------------------------------------------------
// 1. Mock Keyring Provider (Used in CI, Unit Tests, and Sandbox Environments)
// ---------------------------------------------------------------------------
use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

type StoredKey = (EnclaveKeyInfo, Vec<u8>);
type InMemoryKeyStore = Arc<RwLock<HashMap<String, StoredKey>>>;

#[derive(Default)]
pub struct MockKeyringProvider {
    // Stores ephemeral Ed25519 keypairs in memory
    keys: InMemoryKeyStore,
    pub simulate_biometric_delay_ms: u64,
}

impl MockKeyringProvider {
    pub fn new() -> Self {
        Self {
            keys: Arc::new(RwLock::new(HashMap::new())),
            simulate_biometric_delay_ms: 0,
        }
    }

    pub async fn add_ephemeral_key(&self, key_id: &str) -> Result<EnclaveKeyInfo, KeyringError> {
        let rng = SystemRandom::new();
        let pkcs8_bytes = Ed25519KeyPair::generate_pkcs8(&rng)
            .map_err(|e| KeyringError::SigningError(e.to_string()))?;

        let key_pair = Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref())
            .map_err(|e| KeyringError::SigningError(e.to_string()))?;

        let info = EnclaveKeyInfo {
            key_id: key_id.to_string(),
            algorithm: "ssh-ed25519".to_string(),
            public_key: key_pair.public_key().as_ref().to_vec(),
            requires_touch: true,
        };

        self.keys.write().await.insert(
            key_id.to_string(),
            (info.clone(), pkcs8_bytes.as_ref().to_vec()),
        );
        Ok(info)
    }
}

#[async_trait]
impl KeyringProvider for MockKeyringProvider {
    async fn list_keys(&self) -> Result<Vec<EnclaveKeyInfo>, KeyringError> {
        let map = self.keys.read().await;
        Ok(map.values().map(|(info, _)| info.clone()).collect())
    }

    async fn sign(&self, key_id: &str, data: &[u8]) -> Result<Vec<u8>, KeyringError> {
        if self.simulate_biometric_delay_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(
                self.simulate_biometric_delay_ms,
            ))
            .await;
        }

        let map = self.keys.read().await;
        let (_, pkcs8) = map.get(key_id).ok_or(KeyringError::KeyNotFound)?;

        let key_pair = Ed25519KeyPair::from_pkcs8(pkcs8)
            .map_err(|e| KeyringError::SigningError(e.to_string()))?;

        let signature = key_pair.sign(data);
        Ok(signature.as_ref().to_vec())
    }
}

// ---------------------------------------------------------------------------
// 2. System Keyring Provider (OS Keychain / Secret Service / Credential Manager)
// ---------------------------------------------------------------------------

pub struct SystemKeyringProvider {
    pub service_name: String,
}

impl SystemKeyringProvider {
    pub fn new(service_name: &str) -> Self {
        Self {
            service_name: service_name.to_string(),
        }
    }

    pub fn get_secret(&self, key_id: &str) -> Result<String, KeyringError> {
        let entry = keyring::Entry::new(&self.service_name, key_id)
            .map_err(|e| KeyringError::AuthFailed(e.to_string()))?;
        entry
            .get_password()
            .map_err(|e| KeyringError::AuthFailed(e.to_string()))
    }

    pub fn set_secret(&self, key_id: &str, secret: &str) -> Result<(), KeyringError> {
        let entry = keyring::Entry::new(&self.service_name, key_id)
            .map_err(|e| KeyringError::AuthFailed(e.to_string()))?;
        entry
            .set_password(secret)
            .map_err(|e| KeyringError::AuthFailed(e.to_string()))
    }

    pub fn delete_secret(&self, key_id: &str) -> Result<(), KeyringError> {
        let entry = keyring::Entry::new(&self.service_name, key_id)
            .map_err(|e| KeyringError::AuthFailed(e.to_string()))?;
        entry
            .delete_credential()
            .map_err(|e| KeyringError::AuthFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_keyring_provider() {
        let provider = MockKeyringProvider::new();
        let key_info = provider.add_ephemeral_key("test_key").await.unwrap();
        assert_eq!(key_info.key_id, "test_key");

        let signature = provider.sign("test_key", b"test_data").await.unwrap();
        assert!(!signature.is_empty());
    }
}
