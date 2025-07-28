// SPDX-License-Identifier: MPL-2.0

use cosmic::cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, CosmicConfigEntry};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 1]
pub struct Config {
    pub bluesky: BlueskyConfig,
    pub mastodon: MastodonConfig,
    pub nostr: NostrConfig,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct BlueskyConfig {
    pub enabled: bool,
    pub handle: String,
    pub password: String, // App password
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct MastodonConfig {
    pub enabled: bool,
    pub instance_url: String,
    pub access_token: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct NostrConfig {
    pub enabled: bool,
    pub private_key: String, // Hex encoded private key
    pub relays: Vec<String>,
}
