// SPDX-License-Identifier: MPL-2.0

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zeroize::Zeroize;

#[derive(Debug, Clone)]
pub enum CryptoError {
    EncryptionFailed,
    DecryptionFailed,
    KeyDerivationFailed,
    InvalidData,
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CryptoError::EncryptionFailed => write!(f, "Encryption failed"),
            CryptoError::DecryptionFailed => write!(f, "Decryption failed"),
            CryptoError::KeyDerivationFailed => write!(f, "Key derivation failed"),
            CryptoError::InvalidData => write!(f, "Invalid encrypted data"),
        }
    }
}

impl std::error::Error for CryptoError {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptedData {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub salt: String,
}

impl Zeroize for EncryptedData {
    fn zeroize(&mut self) {
        self.ciphertext.zeroize();
        self.nonce.zeroize();
        self.salt.zeroize();
    }
}

impl Drop for EncryptedData {
    fn drop(&mut self) {
        self.zeroize();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecureCredentials {
    pub encrypted_data: HashMap<String, EncryptedData>,
}

pub struct CryptoManager {
    master_key: Option<Key<Aes256Gcm>>,
}

impl CryptoManager {
    pub fn new() -> Self {
        Self { master_key: None }
    }

    /// Initialize with a master password (derived from user input)
    /// Currently unused but available for future master password feature
    #[allow(dead_code)]
    pub fn init_with_password(&mut self, password: &str) -> Result<(), CryptoError> {
        // Generate a random salt for key derivation
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        
        // Derive key from password
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| CryptoError::KeyDerivationFailed)?;
        
        // Use the hash as our encryption key (first 32 bytes)
        let hash = password_hash.hash.unwrap();
        let key_bytes = hash.as_bytes();
        if key_bytes.len() < 32 {
            return Err(CryptoError::KeyDerivationFailed);
        }
        
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes[..32]);
        self.master_key = Some(*key);
        
        Ok(())
    }

    /// Generate a master key from machine-specific data (fallback when no password is set)
    pub fn init_with_machine_key(&mut self) -> Result<(), CryptoError> {
        // Use machine-specific data as entropy
        let machine_id = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "default-machine".to_string());
        
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "default-user".to_string());
        
        let combined = format!("yall-cosmic-{}-{}", machine_id, user);
        
        // Derive key using Argon2
        let salt = SaltString::from_b64("c29tZXNhbHR2YWx1ZQ").unwrap(); // Fixed salt for machine keys
        let argon2 = Argon2::default();
        
        let password_hash = argon2
            .hash_password(combined.as_bytes(), &salt)
            .map_err(|_| CryptoError::KeyDerivationFailed)?;
        
        let hash = password_hash.hash.unwrap();
        let key_bytes = hash.as_bytes();
        if key_bytes.len() < 32 {
            return Err(CryptoError::KeyDerivationFailed);
        }
        
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes[..32]);
        self.master_key = Some(*key);
        
        Ok(())
    }

    /// Encrypt a credential value
    pub fn encrypt(&self, plaintext: &str) -> Result<EncryptedData, CryptoError> {
        let key = self.master_key.ok_or(CryptoError::EncryptionFailed)?;
        let cipher = Aes256Gcm::new(&key);
        
        // Generate random nonce
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        
        // Encrypt the data
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|_| CryptoError::EncryptionFailed)?;
        
        // Generate salt for this specific encryption
        let salt = SaltString::generate(&mut OsRng);
        
        Ok(EncryptedData {
            ciphertext,
            nonce: nonce.to_vec(),
            salt: salt.to_string(),
        })
    }

    /// Decrypt a credential value
    pub fn decrypt(&self, encrypted: &EncryptedData) -> Result<String, CryptoError> {
        let key = self.master_key.ok_or(CryptoError::DecryptionFailed)?;
        let cipher = Aes256Gcm::new(&key);
        
        // Reconstruct nonce
        if encrypted.nonce.len() != 12 {
            return Err(CryptoError::InvalidData);
        }
        let nonce = Nonce::from_slice(&encrypted.nonce);
        
        // Decrypt the data
        let plaintext = cipher
            .decrypt(nonce, encrypted.ciphertext.as_ref())
            .map_err(|_| CryptoError::DecryptionFailed)?;
        
        String::from_utf8(plaintext).map_err(|_| CryptoError::InvalidData)
    }
}

impl Drop for CryptoManager {
    fn drop(&mut self) {
        if let Some(mut key) = self.master_key.take() {
            key.zeroize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let mut crypto = CryptoManager::new();
        crypto.init_with_machine_key().unwrap();
        
        let plaintext = "secret-token-123";
        let encrypted = crypto.encrypt(plaintext).unwrap();
        let decrypted = crypto.decrypt(&encrypted).unwrap();
        
        assert_eq!(plaintext, decrypted);
    }
}