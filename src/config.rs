// SPDX-License-Identifier: MPL-2.0

use cosmic::cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};
use serde::{Deserialize, Serialize};
use crate::crypto::{EncryptedData, CryptoManager, CryptoError};

#[derive(Debug, Default, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 1]
pub struct Config {
    pub bluesky: BlueskyConfig,
    pub mastodon: MastodonConfig,
    pub nostr: NostrConfig,
    pub microblog: MicroBlogConfig,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct MicroBlogConfig {
    pub enabled: bool,
    pub access_token: Option<EncryptedData>, // Encrypted token
    #[serde(skip)]
    pub decrypted_access_token: String, // Runtime-only decrypted value
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct BlueskyConfig {
    pub enabled: bool,
    pub handle: String, // Handle is not sensitive, can be stored in plain text
    pub password: Option<EncryptedData>, // Encrypted app password
    #[serde(skip)]
    pub decrypted_password: String, // Runtime-only decrypted value
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct MastodonConfig {
    pub enabled: bool,
    pub instance_url: String, // URL is not sensitive
    pub access_token: Option<EncryptedData>, // Encrypted token
    #[serde(skip)]
    pub decrypted_access_token: String, // Runtime-only decrypted value
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct NostrConfig {
    pub enabled: bool,
    pub private_key: Option<EncryptedData>, // Encrypted private key
    pub relays: Vec<String>, // Relay URLs are not sensitive
    #[serde(skip)]
    pub decrypted_private_key: String, // Runtime-only decrypted value
}

impl Default for NostrConfig {
    fn default() -> Self {
        NostrConfig {
            enabled: false,
            private_key: None,
            relays: vec![
                "wss://relay.damus.io".to_string(),
                "wss://nos.lol".to_string(),
                "wss://relay.snort.social".to_string(),
                "wss://nostr.wine".to_string(),
            ],
            decrypted_private_key: String::new(),
        }
    }
}

impl Config {
    /// Decrypt all credentials using the provided crypto manager
    pub fn decrypt_credentials(&mut self, crypto: &CryptoManager) -> Result<(), CryptoError> {
        // Decrypt Bluesky password
        if let Some(encrypted_password) = &self.bluesky.password {
            self.bluesky.decrypted_password = crypto.decrypt(encrypted_password)?;
        }

        // Decrypt Mastodon token
        if let Some(encrypted_token) = &self.mastodon.access_token {
            self.mastodon.decrypted_access_token = crypto.decrypt(encrypted_token)?;
        }

        // Decrypt Micro.Blog token
        if let Some(encrypted_token) = &self.microblog.access_token {
            self.microblog.decrypted_access_token = crypto.decrypt(encrypted_token)?;
        }

        // Decrypt Nostr private key
        if let Some(encrypted_key) = &self.nostr.private_key {
            self.nostr.decrypted_private_key = crypto.decrypt(encrypted_key)?;
        }

        Ok(())
    }

    /// Encrypt credentials before saving
    pub fn encrypt_credentials(&mut self, crypto: &CryptoManager) -> Result<(), CryptoError> {
        // Encrypt Bluesky password
        if !self.bluesky.decrypted_password.is_empty() {
            self.bluesky.password = Some(crypto.encrypt(&self.bluesky.decrypted_password)?);
        }

        // Encrypt Mastodon token
        if !self.mastodon.decrypted_access_token.is_empty() {
            self.mastodon.access_token = Some(crypto.encrypt(&self.mastodon.decrypted_access_token)?);
        }

        // Encrypt Micro.Blog token
        if !self.microblog.decrypted_access_token.is_empty() {
            self.microblog.access_token = Some(crypto.encrypt(&self.microblog.decrypted_access_token)?);
        }

        // Encrypt Nostr private key
        if !self.nostr.decrypted_private_key.is_empty() {
            self.nostr.private_key = Some(crypto.encrypt(&self.nostr.decrypted_private_key)?);
        }

        Ok(())
    }
}
