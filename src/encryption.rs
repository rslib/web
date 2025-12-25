use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit},
};
use anyhow::{Context, Result, anyhow};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::RngCore;
use std::process::Command;

use crate::config::EncryptionConfig;

/// Argon2 parameters - must match JavaScript implementation
const ARGON2_MEMORY_COST: u32 = 65536; // 64 MiB
const ARGON2_TIME_COST: u32 = 3;
const ARGON2_PARALLELISM: u32 = 1;
const SALT_LENGTH: usize = 16;
const NONCE_LENGTH: usize = 12;
const KEY_LENGTH: usize = 32; // AES-256

/// Encrypted content with all data needed for decryption
#[derive(Debug, Clone)]
pub struct EncryptedContent {
    /// Base64-encoded ciphertext
    pub ciphertext: String,
    /// Base64-encoded salt used for key derivation
    pub salt: String,
    /// Base64-encoded nonce used for encryption
    pub nonce: String,
}

/// Resolve password from various sources in priority order:
/// 1. SITE_PASSWORD environment variable
/// 2. password_command output
/// 3. config password
/// 4. per-post password (frontmatter)
pub fn resolve_password(
    config: &EncryptionConfig,
    frontmatter_password: Option<&str>,
) -> Result<String> {
    // Priority 1: Environment variable
    if let Ok(password) = std::env::var("SITE_PASSWORD") {
        if !password.is_empty() {
            return Ok(password);
        }
    }

    // Priority 2: Command output
    if let Some(ref cmd) = config.password_command {
        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .with_context(|| format!("Failed to execute password command: {}", cmd))?;

        if output.status.success() {
            let password = String::from_utf8(output.stdout)
                .with_context(|| "Password command output is not valid UTF-8")?
                .trim()
                .to_string();
            if !password.is_empty() {
                return Ok(password);
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "Password command failed: {} - {}",
                cmd,
                stderr.trim()
            ));
        }
    }

    // Priority 3: Config password
    if let Some(ref password) = config.password {
        return Ok(password.clone());
    }

    // Priority 4: Frontmatter password
    if let Some(password) = frontmatter_password {
        return Ok(password.to_string());
    }

    Err(anyhow!(
        "No encryption password found. Set SITE_PASSWORD env var, \
         configure password_command, or set password in config/frontmatter"
    ))
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
    // Generate random salt and nonce
    let mut salt = [0u8; SALT_LENGTH];
    let mut nonce_bytes = [0u8; NONCE_LENGTH];
    rand::thread_rng().fill_bytes(&mut salt);
    rand::thread_rng().fill_bytes(&mut nonce_bytes);

    // Derive key from password
    let key = derive_key(password, &salt)?;

    // Create cipher and encrypt
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, content.as_bytes())
        .map_err(|e| anyhow!("Encryption failed: {}", e))?;

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

    #[test]
    fn test_resolve_password_from_frontmatter() {
        let config = EncryptionConfig {
            password_command: None,
            password: None,
        };

        let password = resolve_password(&config, Some("frontmatter-pass")).unwrap();
        assert_eq!(password, "frontmatter-pass");
    }

    #[test]
    fn test_resolve_password_from_config() {
        let config = EncryptionConfig {
            password_command: None,
            password: Some("config-pass".to_string()),
        };

        // Config password takes priority over frontmatter
        let password = resolve_password(&config, Some("frontmatter-pass")).unwrap();
        assert_eq!(password, "config-pass");
    }

    #[test]
    fn test_resolve_password_no_source() {
        let config = EncryptionConfig {
            password_command: None,
            password: None,
        };

        let result = resolve_password(&config, None);
        assert!(result.is_err());
    }
}
