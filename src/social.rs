// SPDX-License-Identifier: MPL-2.0

use crate::config::{BlueskyConfig, MastodonConfig, NostrConfig, MicroBlogConfig};
use reqwest::multipart;
use serde_json::{json, Value};
use std::error::Error;
use std::fmt;

use nostr_sdk::prelude::*;
use nostr_sdk::Client as NostrClient;

const BLUESKY_CHARACTER_LIMIT: usize = 300;

#[derive(Debug, Clone)]
pub enum PostError {
    Network(String),
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
        PostError::Network(error.to_string())
    }
}

pub async fn post_to_bluesky(config: &BlueskyConfig, text: &str, image_path: Option<&str>) -> Result<(), PostError> {
    if !config.enabled || config.handle.is_empty() || config.decrypted_password.is_empty() {
        return Err(PostError::Auth("Bluesky not configured".to_string()));
    }

    // Fallback: If text is empty and image is present, set to a single space
    let text = if text.trim().is_empty() && image_path.is_some() {
        " "
    } else {
        text
    };
    // Truncate text to Bluesky's character limit (respecting Unicode boundaries)
    let truncated_text = if text.chars().count() > BLUESKY_CHARACTER_LIMIT {
        text.chars().take(BLUESKY_CHARACTER_LIMIT).collect::<String>()
    } else {
        text.to_string()
    };

    let client = reqwest::Client::new();
    // Create session
    let auth_response = client
        .post("https://bsky.social/xrpc/com.atproto.server.createSession")
        .json(&json!({
            "identifier": config.handle,
            "password": config.decrypted_password
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

    // Upload image if provided (raw bytes, correct headers)
    let mut image_blob = None;
    if let Some(path) = image_path {
        let img_bytes = std::fs::read(path).map_err(|e| PostError::Api(format!("Failed to read image: {}", e)))?;
        if img_bytes.len() > 1_000_000 {
            return Err(PostError::Api(format!("Image file size too large. 1000000 bytes maximum, got: {}", img_bytes.len())));
        }
        let mime_type = mime_guess::from_path(path).first_or_octet_stream().to_string();
        let upload_response = client
            .post("https://bsky.social/xrpc/com.atproto.repo.uploadBlob")
            .header("Authorization", format!("Bearer {}", access_jwt))
            .header("Content-Type", &mime_type)
            .body(img_bytes.clone())
            .send()
            .await?;
        eprintln!("[Bluesky] Image upload status: {}", upload_response.status());
        if upload_response.status().is_success() {
            let upload_data: Value = upload_response.json().await?;
            eprintln!("[Bluesky] Image upload response: {}", upload_data);
            if let Some(blob) = upload_data.get("blob") {
                image_blob = Some(json!({
                    "$type": "blob",
                    "ref": blob["ref"].clone(),
                    "mimeType": blob["mimeType"].clone(),
                    "size": blob["size"].clone()
                }));
            }
        } else {
            let err_text = upload_response.text().await.unwrap_or_default();
            eprintln!("[Bluesky] Image upload failed: {}", err_text);
        }
    }

    // Create post
    let now = chrono::Utc::now().to_rfc3339();
    let mut record = json!({
        "text": truncated_text,
        "createdAt": now,
        "$type": "app.bsky.feed.post"
    });
    if let Some(blob) = image_blob {
        record["embed"] = json!({
            "$type": "app.bsky.embed.images",
            "images": [{
                "image": blob,
                "alt": ""
            }]
        });
    }
    let post_json = json!({
        "repo": config.handle,
        "collection": "app.bsky.feed.post",
        "record": record
    });
    eprintln!("[Bluesky] Post JSON: {}", post_json);
    let post_response = client
        .post("https://bsky.social/xrpc/com.atproto.repo.createRecord")
        .header("Authorization", format!("Bearer {}", access_jwt))
        .json(&post_json)
        .send()
        .await?;
    eprintln!("[Bluesky] Post status: {}", post_response.status());
    if !post_response.status().is_success() {
        let error_text = post_response.text().await.unwrap_or_default();
        eprintln!("[Bluesky] Post failed: {}", error_text);
        return Err(PostError::Api(format!("Bluesky API error: {}", error_text)));
    }
    Ok(())
}

pub async fn post_to_mastodon(config: &MastodonConfig, text: &str, image_path: Option<&str>) -> Result<(), PostError> {
    if !config.enabled || config.instance_url.is_empty() || config.decrypted_access_token.is_empty() {
        return Err(PostError::Auth("Mastodon not configured".to_string()));
    }

    let client = reqwest::Client::new();
    let url = format!("{}/api/v1/statuses", config.instance_url.trim_end_matches('/'));
    let mut media_id = None;
    if let Some(path) = image_path {
        let file_name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "image".to_string());
        let img_bytes = std::fs::read(path).map_err(|e| PostError::Api(format!("Failed to read image: {}", e)))?;
        let part = multipart::Part::bytes(img_bytes).file_name(file_name);
        let form = multipart::Form::new().part("file", part);
        let media_resp = client
            .post(&format!("{}/api/v2/media", config.instance_url.trim_end_matches('/')))
            .header("Authorization", format!("Bearer {}", config.decrypted_access_token))
            .multipart(form)
            .send()
            .await?;
        if media_resp.status().is_success() {
            let media_json: Value = media_resp.json().await?;
            media_id = media_json["id"].as_str().map(|s| s.to_string());
        }
    }
    let mut form = vec![("status", text.to_string())];
    if let Some(id) = media_id {
        form.push(("media_ids[]", id));
    }
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", config.decrypted_access_token))
        .form(&form)
        .send()
        .await?;
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(PostError::Api(format!("Mastodon API error: {}", error_text)));
    }
    Ok(())
}

pub async fn post_to_nostr(config: &NostrConfig, text: &str, _image_path: Option<&str>) -> Result<(), PostError> {
    if !config.enabled || config.decrypted_private_key.is_empty() || config.relays.is_empty() {
        return Err(PostError::Auth("Nostr not configured".to_string()));
    }

    // Parse private key
    let secret_key = match SecretKey::from_hex(&config.decrypted_private_key) {
        Ok(sk) => sk,
        Err(e) => return Err(PostError::Crypto(format!("Invalid private key: {}", e))),
    };
    let keys = Keys::new(secret_key);

    // Set up relay pool
    let nostr_client = NostrClient::new(keys.clone());
    for relay in &config.relays {
        let _ = nostr_client.add_relay(relay).await;
    }
    nostr_client.connect().await;

    // Image posting is disabled for Nostr
    let post_text = text.to_string();
    // Create and send event
    let pubkey = keys.public_key();
    let unsigned = EventBuilder::text_note(&post_text).build(pubkey);
    let event = keys.sign_event(unsigned).await.map_err(|e| PostError::Crypto(format!("Failed to sign event: {}", e)))?;
    eprintln!("[Nostr] Final event: {:?}", event);
    let send_result = nostr_client.send_event(&event).await;
    if send_result.is_err() {
        let err = send_result.err();
        eprintln!("[Nostr] Failed to post to any relays: {:?}", err);
        return Err(PostError::Api(format!("Failed to post to any Nostr relays: {:?}", err)));
    }
    Ok(())
}

pub async fn post_to_microblog(config: &MicroBlogConfig, text: &str, image_path: Option<&str>) -> Result<(), PostError> {
    if !config.enabled || config.decrypted_access_token.is_empty() {
        return Err(PostError::Auth("Micro.Blog not configured".to_string()));
    }

    let client = reqwest::Client::new();
    if let Some(path) = image_path {
        let img_bytes = std::fs::read(path).map_err(|e| PostError::Api(format!("Failed to read image: {}", e)))?;
        let part = reqwest::multipart::Part::bytes(img_bytes).file_name("image.jpg");
        let content_owned = text.to_string();
        let form_data = reqwest::multipart::Form::new()
            .text("h", "entry")
            .text("content", content_owned)
            .part("photo", part);
        let response = client
            .post("https://micro.blog/micropub")
            .header("Authorization", format!("Bearer {}", config.decrypted_access_token))
            .multipart(form_data)
            .send()
            .await?;
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(PostError::Api(format!("Micro.Blog API error: {}", error_text)));
        }
        return Ok(());
    }
    let response = client
        .post("https://micro.blog/micropub")
        .header("Authorization", format!("Bearer {}", config.decrypted_access_token))
        .form(&[("h", "entry"), ("content", text)])
        .send()
        .await?;
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(PostError::Api(format!("Micro.Blog API error: {}", error_text)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bluesky_truncation() {
        // Test that long text gets truncated to 300 characters
        let long_text = "a".repeat(400);
        let truncated = if long_text.chars().count() > BLUESKY_CHARACTER_LIMIT {
            long_text.chars().take(BLUESKY_CHARACTER_LIMIT).collect::<String>()
        } else {
            long_text.to_string()
        };
        
        assert_eq!(truncated.chars().count(), BLUESKY_CHARACTER_LIMIT);
        assert_eq!(truncated, "a".repeat(300));
    }

    #[test]
    fn test_bluesky_no_truncation_needed() {
        // Test that short text doesn't get truncated
        let short_text = "Hello, world!";
        let result = if short_text.chars().count() > BLUESKY_CHARACTER_LIMIT {
            short_text.chars().take(BLUESKY_CHARACTER_LIMIT).collect::<String>()
        } else {
            short_text.to_string()
        };
        
        assert_eq!(result, short_text);
    }

    #[test]
    fn test_unicode_truncation() {
        // Test that Unicode characters are handled properly
        let unicode_text = "🚀".repeat(400); // Each emoji is multiple bytes but 1 character
        let truncated = if unicode_text.chars().count() > BLUESKY_CHARACTER_LIMIT {
            unicode_text.chars().take(BLUESKY_CHARACTER_LIMIT).collect::<String>()
        } else {
            unicode_text.to_string()
        };
        
        assert_eq!(truncated.chars().count(), BLUESKY_CHARACTER_LIMIT);
        assert_eq!(truncated, "🚀".repeat(300));
    }
}