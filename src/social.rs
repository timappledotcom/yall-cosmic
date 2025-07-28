// SPDX-License-Identifier: MPL-2.0

use crate::config::{BlueskyConfig, MastodonConfig, NostrConfig};
use reqwest::Client;
use serde_json::{json, Value};
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum PostError {
    Network(reqwest::Error),
    Auth(String),
    Api(String),
    Crypto(String),
}

impl fmt::Display for PostError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PostError::Network(e) => write!(f, "Network error: {}", e),
            PostError::Auth(e) => write!(f, "Authentication error: {}", e),
            PostError::Api(e) => write!(f, "API error: {}", e),
            PostError::Crypto(e) => write!(f, "Cryptography error: {}", e),
        }
    }
}

impl Error for PostError {}

impl From<reqwest::Error> for PostError {
    fn from(error: reqwest::Error) -> Self {
        PostError::Network(error)
    }
}

pub async fn post_to_bluesky(config: &BlueskyConfig, text: &str) -> Result<(), PostError> {
    if !config.enabled || config.handle.is_empty() || config.password.is_empty() {
        return Err(PostError::Auth("Bluesky not configured".to_string()));
    }

    let client = Client::new();
    
    // Create session
    let auth_response = client
        .post("https://bsky.social/xrpc/com.atproto.server.createSession")
        .json(&json!({
            "identifier": config.handle,
            "password": config.password
        }))
        .send()
        .await?;

    if !auth_response.status().is_success() {
        return Err(PostError::Auth("Failed to authenticate with Bluesky".to_string()));
    }

    let auth_data: Value = auth_response.json().await?;
    let access_jwt = auth_data["accessJwt"]
        .as_str()
        .ok_or_else(|| PostError::Auth("No access token received".to_string()))?;

    // Create post
    let now = chrono::Utc::now().to_rfc3339();
    let post_response = client
        .post("https://bsky.social/xrpc/com.atproto.repo.createRecord")
        .header("Authorization", format!("Bearer {}", access_jwt))
        .json(&json!({
            "repo": config.handle,
            "collection": "app.bsky.feed.post",
            "record": {
                "text": text,
                "createdAt": now,
                "$type": "app.bsky.feed.post"
            }
        }))
        .send()
        .await?;

    if !post_response.status().is_success() {
        let error_text = post_response.text().await.unwrap_or_default();
        return Err(PostError::Api(format!("Bluesky API error: {}", error_text)));
    }

    Ok(())
}

pub async fn post_to_mastodon(config: &MastodonConfig, text: &str) -> Result<(), PostError> {
    if !config.enabled || config.instance_url.is_empty() || config.access_token.is_empty() {
        return Err(PostError::Auth("Mastodon not configured".to_string()));
    }

    let client = Client::new();
    let url = format!("{}/api/v1/statuses", config.instance_url.trim_end_matches('/'));
    
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.access_token))
        .form(&[("status", text)])
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(PostError::Api(format!("Mastodon API error: {}", error_text)));
    }

    Ok(())
}

pub async fn post_to_nostr(config: &NostrConfig, text: &str) -> Result<(), PostError> {
    if !config.enabled || config.private_key.is_empty() || config.relays.is_empty() {
        return Err(PostError::Auth("Nostr not configured".to_string()));
    }

    // Parse private key
    let private_key_bytes = hex::decode(&config.private_key)
        .map_err(|e| PostError::Crypto(format!("Invalid private key: {}", e)))?;
    
    let secret_key = secp256k1::SecretKey::from_slice(&private_key_bytes)
        .map_err(|e| PostError::Crypto(format!("Invalid secret key: {}", e)))?;
    
    let secp = secp256k1::Secp256k1::new();
    let public_key = secp256k1::PublicKey::from_secret_key(&secp, &secret_key);
    let pubkey_hex = hex::encode(public_key.serialize()[1..].to_vec()); // Remove first byte

    // Create Nostr event
    let created_at = chrono::Utc::now().timestamp();
    let event_json = json!([
        0,
        pubkey_hex,
        created_at,
        1, // kind 1 = text note
        [],
        text
    ]);

    // Create event ID (sha256 of serialized event)
    let event_string = serde_json::to_string(&event_json)
        .map_err(|e| PostError::Crypto(format!("Failed to serialize event: {}", e)))?;
    
    let event_id = sha2::Sha256::digest(event_string.as_bytes());
    let event_id_hex = hex::encode(event_id);

    // Sign the event ID
    let message = secp256k1::Message::from_digest_slice(&event_id)
        .map_err(|e| PostError::Crypto(format!("Failed to create message: {}", e)))?;
    
    let signature = secp.sign_ecdsa(&message, &secret_key);
    let signature_hex = hex::encode(signature.serialize_compact());

    // Create final event
    let final_event = json!({
        "id": event_id_hex,
        "pubkey": pubkey_hex,
        "created_at": created_at,
        "kind": 1,
        "tags": [],
        "content": text,
        "sig": signature_hex
    });

    // Send to relays
    let client = Client::new();
    let mut success_count = 0;
    
    for relay_url in &config.relays {
        let relay_message = json!(["EVENT", final_event]);
        
        // For HTTP relays (simplified - real Nostr uses WebSockets)
        let response = client
            .post(relay_url)
            .json(&relay_message)
            .send()
            .await;
            
        if response.is_ok() && response.unwrap().status().is_success() {
            success_count += 1;
        }
    }

    if success_count == 0 {
        return Err(PostError::Api("Failed to post to any Nostr relays".to_string()));
    }

    Ok(())
}