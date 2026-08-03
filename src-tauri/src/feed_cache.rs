//! Cache local do feed inicial — abre rapido na proxima sessao.

use crate::youtube::HomeFeed;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const FEED_CACHE_TTL_SECS: u64 = 36 * 3600;

#[derive(Serialize, Deserialize)]
struct FeedCacheFile {
    saved_at: u64,
    feed: HomeFeed,
}

pub fn load_feed_cache() -> Option<HomeFeed> {
    let path = feed_cache_path();
    let raw = fs::read_to_string(&path).ok()?;
    let file: FeedCacheFile = serde_json::from_str(&raw).ok()?;
    let now = now_secs();
    if now.saturating_sub(file.saved_at) > FEED_CACHE_TTL_SECS {
        let _ = fs::remove_file(&path);
        return None;
    }
    Some(file.feed)
}

pub fn save_feed_cache(feed: &HomeFeed) {
    let path = feed_cache_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let file = FeedCacheFile {
        saved_at: now_secs(),
        feed: feed.clone(),
    };
    if let Ok(json) = serde_json::to_string(&file) {
        let _ = fs::write(&path, json);
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn feed_cache_path() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("promptub").join("feed_cache.json")
}

#[tauri::command]
pub fn save_stored_feed(feed: HomeFeed) {
    save_feed_cache(&feed);
}

#[tauri::command]
pub fn get_stored_feed() -> Option<HomeFeed> {
    load_feed_cache()
}
