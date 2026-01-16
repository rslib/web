use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit},
};
use anyhow::{Result, anyhow};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use log::trace;
use rand::RngCore;

/// Argon2 parameters - must match JavaScript implementation
const ARGON2_MEMORY_COST: u32 = 65536; // 64 MiB
const ARGON2_TIME_COST: u32 = 3;
const ARGON2_PARALLELISM: u32 = 1;
const SALT_LENGTH: usize = 16;
const NONCE_LENGTH: usize = 12;
const KEY_LENGTH: usize = 32; // AES-256

/// Encrypted content with all data needed for decryption
#[derive(Debug, Clone, serde::Serialize)]
pub struct EncryptedContent {
    /// Base64-encoded ciphertext
    pub ciphertext: String,
    /// Base64-encoded salt used for key derivation
    pub salt: String,
    /// Base64-encoded nonce used for encryption
    pub nonce: String,
}

/// Derive a 256-bit key from password using Argon2id
fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; KEY_LENGTH]> {
    let params = Params::new(
        ARGON2_MEMORY_COST,
        ARGON2_TIME_COST,
        ARGON2_PARALLELISM,
        Some(KEY_LENGTH),
    )
    .map_err(|e| anyhow!("Failed to create Argon2 params: {}", e))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; KEY_LENGTH];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| anyhow!("Failed to derive key: {}", e))?;

    Ok(key)
}

/// Encrypt content using AES-256-GCM with Argon2id key derivation
pub fn encrypt_content(content: &str, password: &str) -> Result<EncryptedContent> {
    trace!("Encrypting content ({} bytes)", content.len());

    // Generate random salt and nonce
    let mut salt = [0u8; SALT_LENGTH];
    let mut nonce_bytes = [0u8; NONCE_LENGTH];
    rand::thread_rng().fill_bytes(&mut salt);
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    // Derive key from password
    trace!("Deriving key with Argon2id");
    let key = derive_key(password, &salt)?;

    // Create cipher and encrypt
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, content.as_bytes())
        .map_err(|e| anyhow!("Encryption failed: {}", e))?;

    trace!("Content encrypted successfully");
    Ok(EncryptedContent {
        ciphertext: BASE64.encode(&ciphertext),
        salt: BASE64.encode(salt),
        nonce: BASE64.encode(nonce_bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_content() {
        let content = "This is a secret message!";
        let password = "test-password-123";

        let encrypted = encrypt_content(content, password).unwrap();

        // Verify all fields are base64-encoded and non-empty
        assert!(!encrypted.ciphertext.is_empty());
        assert!(!encrypted.salt.is_empty());
        assert!(!encrypted.nonce.is_empty());

        // Verify we can decode the base64
        let ciphertext_bytes = BASE64.decode(&encrypted.ciphertext).unwrap();
        let salt_bytes = BASE64.decode(&encrypted.salt).unwrap();
        let nonce_bytes = BASE64.decode(&encrypted.nonce).unwrap();

        // Ciphertext should be longer than plaintext (includes auth tag)
        assert!(ciphertext_bytes.len() > content.len());
        assert_eq!(salt_bytes.len(), SALT_LENGTH);
        assert_eq!(nonce_bytes.len(), NONCE_LENGTH);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let content = "Secret content for roundtrip test!";
        let password = "roundtrip-password";

        let encrypted = encrypt_content(content, password).unwrap();

        // Decrypt to verify
        let salt = BASE64.decode(&encrypted.salt).unwrap();
        let nonce_bytes = BASE64.decode(&encrypted.nonce).unwrap();
        let ciphertext = BASE64.decode(&encrypted.ciphertext).unwrap();

        let key = derive_key(password, &salt).unwrap();
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        let nonce = Nonce::from_slice(&nonce_bytes);

        let decrypted = cipher.decrypt(nonce, ciphertext.as_ref()).unwrap();
        let decrypted_str = String::from_utf8(decrypted).unwrap();

        assert_eq!(decrypted_str, content);
    }

    #[test]
    fn test_wrong_password_fails() {
        let content = "Secret content";
        let password = "correct-password";
        let wrong_password = "wrong-password";

        let encrypted = encrypt_content(content, password).unwrap();

        // Try to decrypt with wrong password
        let salt = BASE64.decode(&encrypted.salt).unwrap();
        let nonce_bytes = BASE64.decode(&encrypted.nonce).unwrap();
        let ciphertext = BASE64.decode(&encrypted.ciphertext).unwrap();

        let key = derive_key(wrong_password, &salt).unwrap();
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Should fail to decrypt
        assert!(cipher.decrypt(nonce, ciphertext.as_ref()).is_err());
    }
}
