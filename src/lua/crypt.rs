//! Encryption functions for Lua API
//!
//! Functions: crypt.encrypt, crypt.decrypt, crypt.encrypt_html

use crate::encryption::encrypt_content;
use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit},
};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use mlua::{Lua, Result, Table, Value};

/// Argon2 parameters - must match encryption.rs and JavaScript implementation
const ARGON2_MEMORY_COST: u32 = 65536; // 64 MiB
const ARGON2_TIME_COST: u32 = 3;
const ARGON2_PARALLELISM: u32 = 1;
const KEY_LENGTH: usize = 32; // AES-256

/// Derive a 256-bit key from password using Argon2id
fn derive_key(password: &str, salt: &[u8]) -> std::result::Result<[u8; KEY_LENGTH], String> {
    let params = Params::new(
        ARGON2_MEMORY_COST,
        ARGON2_TIME_COST,
        ARGON2_PARALLELISM,
        Some(KEY_LENGTH),
    )
    .map_err(|e| format!("Failed to create Argon2 params: {}", e))?;

    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; KEY_LENGTH];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| format!("Failed to derive key: {}", e))?;

    Ok(key)
}

/// Decrypt content using AES-256-GCM
fn decrypt_content(
    ciphertext: &str,
    salt: &str,
    nonce: &str,
    password: &str,
) -> std::result::Result<String, String> {
    let salt_bytes = BASE64
        .decode(salt)
        .map_err(|e| format!("Invalid salt: {}", e))?;
    let nonce_bytes = BASE64
        .decode(nonce)
        .map_err(|e| format!("Invalid nonce: {}", e))?;
    let ciphertext_bytes = BASE64
        .decode(ciphertext)
        .map_err(|e| format!("Invalid ciphertext: {}", e))?;

    let key = derive_key(password, &salt_bytes)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let nonce = Nonce::from_slice(&nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext_bytes.as_ref())
        .map_err(|_| "Decryption failed - wrong password or corrupted data".to_string())?;

    String::from_utf8(plaintext).map_err(|e| format!("Invalid UTF-8 in decrypted content: {}", e))
}

/// Resolve password from various sources
fn resolve_password(explicit: Option<String>, global: &Option<String>) -> Option<String> {
    // Priority: explicit > SITE_PASSWORD env > global config
    explicit
        .or_else(|| {
            std::env::var("SITE_PASSWORD")
                .ok()
                .filter(|s| !s.is_empty())
        })
        .or_else(|| global.clone())
}

/// Create the crypt module table
pub fn create_module(lua: &Lua, global_password: Option<String>) -> Result<Table> {
    let crypt_module = lua.create_table()?;

    // crypt.encrypt(content, password?) -> { ciphertext, salt, nonce }
    let global_pw = global_password.clone();
    let encrypt_fn =
        lua.create_function(move |lua, (content, password): (String, Option<String>)| {
            let pw = resolve_password(password, &global_pw).ok_or_else(|| {
                mlua::Error::RuntimeError(
                    "No password provided. Set SITE_PASSWORD env var or pass password argument."
                        .to_string(),
                )
            })?;

            let encrypted = encrypt_content(&content, &pw)
                .map_err(|e| mlua::Error::RuntimeError(format!("Encryption failed: {}", e)))?;

            let result = lua.create_table()?;
            result.set("ciphertext", encrypted.ciphertext)?;
            result.set("salt", encrypted.salt)?;
            result.set("nonce", encrypted.nonce)?;
            Ok(Value::Table(result))
        })?;
    crypt_module.set("encrypt", encrypt_fn)?;

    // crypt.decrypt(data, password?) -> string
    // data can be { ciphertext, salt, nonce } table
    let global_pw = global_password.clone();
    let decrypt_fn = lua.create_function(move |_, (data, password): (Table, Option<String>)| {
        let pw = resolve_password(password, &global_pw).ok_or_else(|| {
            mlua::Error::RuntimeError(
                "No password provided. Set SITE_PASSWORD env var or pass password argument."
                    .to_string(),
            )
        })?;

        let ciphertext: String = data.get("ciphertext")?;
        let salt: String = data.get("salt")?;
        let nonce: String = data.get("nonce")?;

        let plaintext =
            decrypt_content(&ciphertext, &salt, &nonce, &pw).map_err(mlua::Error::RuntimeError)?;

        Ok(plaintext)
    })?;
    crypt_module.set("decrypt", decrypt_fn)?;

    // crypt.encrypt_html(content, options?) -> string
    // options: { password?, slug?, block_id?, own_password? }
    let global_pw = global_password.clone();
    let encrypt_html_fn =
        lua.create_function(move |_, (content, options): (String, Option<Table>)| {
            let explicit_pw = options
                .as_ref()
                .and_then(|t| t.get::<String>("password").ok());
            let pw = resolve_password(explicit_pw, &global_pw).ok_or_else(|| {
                mlua::Error::RuntimeError(
                    "No password provided. Set SITE_PASSWORD env var or pass password in options."
                        .to_string(),
                )
            })?;

            let slug: String = options
                .as_ref()
                .and_then(|t| t.get("slug").ok())
                .unwrap_or_else(|| "page".to_string());

            let block_id: String = options
                .as_ref()
                .and_then(|t| t.get("block_id").ok())
                .unwrap_or_else(|| format!("block-{}", rand::random::<u32>()));

            let own_password: bool = options
                .as_ref()
                .and_then(|t| t.get("own_password").ok())
                .unwrap_or(false);

            let encrypted = encrypt_content(&content, &pw)
                .map_err(|e| mlua::Error::RuntimeError(format!("Encryption failed: {}", e)))?;

            let own_password_attr = if own_password {
                "\n     data-own-password=\"true\""
            } else {
                ""
            };

            let html = format!(
                r#"<div class="encrypted-content encrypted-block"
     data-encrypted="{}"
     data-salt="{}"
     data-nonce="{}"
     data-slug="{}"
     data-block-id="{}"{}>
    <div class="decrypt-prompt">
        <p class="encrypted-message">This section is encrypted.</p>
        <input type="password" placeholder="Enter password..." aria-label="Password">
        <label class="remember-label">
            <input type="checkbox" class="remember">
            Remember for this post
        </label>
        <button type="button">Decrypt</button>
    </div>
</div>"#,
                encrypted.ciphertext,
                encrypted.salt,
                encrypted.nonce,
                slug,
                block_id,
                own_password_attr
            );

            Ok(html)
        })?;
    crypt_module.set("encrypt_html", encrypt_html_fn)?;

    Ok(crypt_module)
}
