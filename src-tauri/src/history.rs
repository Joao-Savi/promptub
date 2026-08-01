//! Historico local de reproducao — persiste entre sessoes.

use crate::youtube::Video;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_RECENT: usize = 64;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WatchEntry {
    pub video: Video,
    pub watched_at: u64,
    pub play_count: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WatchHistory {
    pub last_music: Option<Video>,
    pub last_video: Option<Video>,
    pub recent_music: Vec<WatchEntry>,
    pub recent_video: Vec<WatchEntry>,
}

impl WatchHistory {
    pub fn load() -> Self {
        let path = history_path();
        if !path.is_file() {
            return Self::default();
        }
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = history_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&path, json).map_err(|e| e.to_string())
    }

    pub fn record(&mut self, video: Video, audio_only: bool) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if audio_only {
            self.last_music = Some(video.clone());
            upsert_recent(&mut self.recent_music, video, now);
        } else {
            self.last_video = Some(video.clone());
            upsert_recent(&mut self.recent_video, video, now);
        }
        let _ = self.save();
    }

    pub fn video_seed(&self) -> Option<Video> {
        self.last_video.clone()
    }

    pub fn music_seed(&self) -> Option<Video> {
        self.last_music.clone()
    }

    pub fn top_uploaders(&self, audio_only: bool, limit: usize) -> Vec<String> {
        let recent = if audio_only {
            &self.recent_music
        } else {
            &self.recent_video
        };
        let mut counts: HashMap<String, u32> = HashMap::new();
        for entry in recent {
            let u = entry.video.uploader.trim();
            if u.len() > 2 {
                *counts.entry(u.to_string()).or_insert(0) += entry.play_count;
            }
        }
        let mut ranked: Vec<_> = counts.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1));
        ranked.into_iter().take(limit).map(|(u, _)| u).collect()
    }

    pub fn frequent_video_ids(&self, limit: usize) -> Vec<String> {
        rank_ids(&self.recent_video, limit)
    }

    pub fn recent_videos(&self, limit: usize) -> Vec<Video> {
        self.recent_video
            .iter()
            .take(limit)
            .map(|e| e.video.clone())
            .collect()
    }

    pub fn uploader_score(&self, uploader: &str, audio_only: bool) -> f32 {
        let key = uploader.trim().to_lowercase();
        if key.len() < 3 {
            return 0.0;
        }
        let recent = if audio_only {
            &self.recent_music
        } else {
            &self.recent_video
        };
        let total: u32 = recent.iter().map(|e| e.play_count).sum();
        if total == 0 {
            return 0.0;
        }
        let mine: u32 = recent
            .iter()
            .filter(|e| e.video.uploader.trim().to_lowercase() == key)
            .map(|e| e.play_count)
            .sum();
        (mine as f32 / total as f32).min(1.0)
    }

    /// Palavras-tema extraidas do que o usuario mais assiste (estilo interesses do YouTube).
    pub fn interest_keywords(&self, audio_only: bool, limit: usize) -> Vec<String> {
        const STOP: &[&str] = &[
            "video", "videos", "oficial", "completo", "parte", "full", "the", "and", "for",
            "com", "para", "sobre", "como", "que", "uma", "por", "dos", "das", "nos", "nas",
            "this", "from", "with", "your", "what", "when", "where", "why", "who",
        ];
        let recent = if audio_only {
            &self.recent_music
        } else {
            &self.recent_video
        };
        let mut counts: HashMap<String, u32> = HashMap::new();
        for entry in recent.iter().take(24) {
            let title = crate::discover::simplify_for_search(&entry.video.title);
            for w in title.split_whitespace() {
                if w.len() < 4 || STOP.contains(&w) {
                    continue;
                }
                *counts.entry(w.to_string()).or_insert(0) += entry.play_count;
            }
        }
        let mut ranked: Vec<_> = counts.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1));
        ranked.into_iter().take(limit).map(|(w, _)| w).collect()
    }
}

fn upsert_recent(list: &mut Vec<WatchEntry>, video: Video, now: u64) {
    if let Some(entry) = list.iter_mut().find(|e| e.video.id == video.id) {
        entry.play_count += 1;
        entry.watched_at = now;
        entry.video = video;
    } else {
        list.insert(
            0,
            WatchEntry {
                video,
                watched_at: now,
                play_count: 1,
            },
        );
    }
    list.sort_by(|a, b| b.watched_at.cmp(&a.watched_at));
    list.truncate(MAX_RECENT);
}

fn rank_ids(recent: &[WatchEntry], limit: usize) -> Vec<String> {
    let mut ranked: Vec<_> = recent
        .iter()
        .map(|e| (e.video.id.clone(), e.play_count))
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1));
    ranked.into_iter().take(limit).map(|(id, _)| id).collect()
}

fn history_path() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("promptub").join("history.json")
}

pub fn hydrate_state(state: &crate::state::AppState, history: &WatchHistory) {
    if state.last_watch_video.lock().is_none() {
        if let Some(v) = history.last_video.clone() {
            *state.last_watch_video.lock() = Some(v);
        }
    }
    if state.last_music_video.lock().is_none() {
        if let Some(v) = history.last_music.clone() {
            *state.last_music_video.lock() = Some(v);
        }
    }
    if state.last_video.lock().is_none() {
        if let Some(v) = history
            .last_video
            .clone()
            .or_else(|| history.last_music.clone())
        {
            *state.last_video.lock() = Some(v);
        }
    }
}
