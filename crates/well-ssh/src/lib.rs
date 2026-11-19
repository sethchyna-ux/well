pub mod client;
pub mod keyring;
pub mod server;

/// Validate an OpenSSH public key and compute the same fingerprint as ssh-keygen.
pub fn public_key_fingerprint(line: &str) -> Result<String, String> {
    let key = russh::keys::PublicKey::from_openssh(line).map_err(|e| e.to_string())?;
    Ok(key.fingerprint(russh::keys::HashAlg::Sha256).to_string())
}
