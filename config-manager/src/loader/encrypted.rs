//! Encrypted configuration loader wrapper.

use crate::core::{Error, Result};
use crate::loader::ConfigLoader;
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::Argon2;
use rand::RngCore;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt::Debug;

/// Encrypted configuration loader wrapper.
///
/// Wraps any `ConfigLoader` and adds AES-256-GCM encryption/decryption.
/// Uses Argon2 for key derivation from passwords.
///
/// # Security
///
/// - Uses AES-256-GCM for authenticated encryption
/// - Random nonces for each encryption operation
/// - Argon2id for password-based key derivation
/// - Constant-time operations where possible
///
/// # Examples
///
/// ```rust,no_run
/// use config_manager::loader::{EncryptedLoader, JsonLoader, ConfigLoader};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, Serialize, Deserialize)]
/// struct SecretConfig {
///     api_key: String,
///     password: String,
/// }
///
/// fn main() -> config_manager::core::Result<()> {
///     // Wrap a JSON loader with encryption
///     let base_loader = JsonLoader::new("secrets.json");
///     let loader = EncryptedLoader::new(base_loader, "my-secret-password");
///
///     let config = SecretConfig {
///         api_key: "secret123".to_string(),
///         password: "pass456".to_string(),
///     };
///
///     // Saves encrypted data to secrets.json
///     loader.save(&config)?;
///
///     // Loads and decrypts
///     let loaded: SecretConfig = loader.load()?;
///
///     Ok(())
/// }
/// ```
pub struct EncryptedLoader<L> {
    inner: L,
    password: String,
}

impl<L> EncryptedLoader<L> {
    /// Create a new encrypted loader wrapping the given loader.
    ///
    /// # Arguments
    ///
    /// * `inner` - The underlying loader to wrap
    /// * `password` - Password for encryption/decryption
    pub fn new(inner: L, password: impl Into<String>) -> Self {
        Self {
            inner,
            password: password.into(),
        }
    }

    /// Derive encryption key from password using Argon2.
    fn derive_key(&self) -> Result<[u8; 32]> {
        // Use a fixed salt for deterministic key derivation
        // In production, you might want to store the salt with the encrypted data
        const SALT_BYTES: [u8; 16] = [
            0x64, 0x65, 0x76, 0x63, 0x6c, 0x69, 0x2d, 0x63, 0x6f, 0x6e, 0x66, 0x69, 0x67, 0x2d,
            0x76, 0x31,
        ]; // "devcli-config-v1"

        let argon2 = Argon2::default();

        // Hash the password with the salt to get a 32-byte key
        let mut key = [0u8; 32];
        argon2
            .hash_password_into(self.password.as_bytes(), &SALT_BYTES, &mut key)
            .map_err(|e| Error::Custom(format!("Failed to derive key: {}", e)))?;

        Ok(key)
    }

    /// Encrypt data using AES-256-GCM.
    fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        let key = self.derive_key()?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| Error::Custom(format!("Failed to create cipher: {}", e)))?;

        // Generate random nonce
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = cipher
            .encrypt(nonce, data)
            .map_err(|e| Error::Custom(format!("Encryption failed: {}", e)))?;

        // Prepend nonce to ciphertext
        let mut result = nonce_bytes.to_vec();
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Decrypt data using AES-256-GCM.
    fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 12 {
            return Err(Error::Custom("Encrypted data too short".to_string()));
        }

        let key = self.derive_key()?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| Error::Custom(format!("Failed to create cipher: {}", e)))?;

        // Extract nonce and ciphertext
        let (nonce_bytes, ciphertext) = data.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt
        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| Error::Custom(format!("Decryption failed: {}", e)))
    }
}

impl<L> Debug for EncryptedLoader<L>
where
    L: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptedLoader")
            .field("inner", &self.inner)
            .field("password", &"<redacted>")
            .finish()
    }
}

impl<T, L> ConfigLoader<T> for EncryptedLoader<L>
where
    T: DeserializeOwned + Serialize,
    L: ConfigLoader<Vec<u8>>,
{
    fn load(&self) -> Result<T> {
        // Load encrypted bytes
        let encrypted = self.inner.load()?;

        // Decrypt
        let decrypted = self.decrypt(&encrypted)?;

        // Deserialize
        serde_json::from_slice(&decrypted)
            .map_err(|e| Error::LoadError(format!("Failed to deserialize: {}", e)))
    }

    fn save(&self, config: &T) -> Result<()> {
        // Serialize
        let data = serde_json::to_vec(config)
            .map_err(|e| Error::SaveError(format!("Failed to serialize: {}", e)))?;

        // Encrypt
        let encrypted = self.encrypt(&data)?;

        // Save encrypted bytes
        self.inner.save(&encrypted)
    }

    fn exists(&self) -> bool {
        self.inner.exists()
    }

    fn source_info(&self) -> String {
        format!("{} (encrypted)", self.inner.source_info())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::JsonLoader;
    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestConfig {
        secret: String,
        value: u32,
    }

    #[test]
    fn test_encrypted_loader_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("encrypted.json");

        // Create encrypted loader wrapping JSON loader
        let base_loader = JsonLoader::new(&config_path);
        let loader = EncryptedLoader::new(base_loader, "test-password");

        let config = TestConfig {
            secret: "my-secret-value".to_string(),
            value: 42,
        };

        // Save encrypted
        loader.save(&config).unwrap();
        assert!(config_path.exists());

        // Verify file contains encrypted data (not readable JSON)
        let raw_content = std::fs::read_to_string(&config_path).unwrap();
        assert!(!raw_content.contains("my-secret-value"));

        // Load and decrypt
        let loaded: TestConfig = loader.load().unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn test_encrypted_loader_wrong_password() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("encrypted.json");

        let config = TestConfig {
            secret: "my-secret".to_string(),
            value: 42,
        };

        // Save with one password
        let base_loader = JsonLoader::new(&config_path);
        let loader1 = EncryptedLoader::new(base_loader, "password1");
        loader1.save(&config).unwrap();

        // Try to load with different password
        let base_loader2 = JsonLoader::new(&config_path);
        let loader2 = EncryptedLoader::new(base_loader2, "wrong-password");
        let result: Result<TestConfig> = loader2.load();

        assert!(result.is_err());
    }

    #[test]
    fn test_encrypted_loader_source_info() {
        let base_loader = JsonLoader::new("/path/to/config.json");
        let loader = EncryptedLoader::new(base_loader, "password");

        let info = ConfigLoader::<TestConfig>::source_info(&loader);
        assert!(info.contains("encrypted"));
        assert!(info.contains("/path/to/config.json"));
    }
}
