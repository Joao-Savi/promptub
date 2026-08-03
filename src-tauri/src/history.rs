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
    pub recent_music: Vec<WatchEntry>,
    #[serde(default)]
    pub last_search: String,
    #[serde(default)]
    pub recent_searches: Vec<String>,
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

    pub fn record(&mut self, video: Video) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.last_music = Some(video.clone());
        upsert_recent(&mut self.recent_music, video, now);
        let _ = self.save();
    }

    pub fn record_search(&mut self, query: &str) {
        let q = query.trim().to_string();
        if q.is_empty() || is_probably_video_id(&q) {
            return;
        }
        self.last_search = q.clone();
        self.recent_searches.retain(|s| s != &q);
        self.recent_searches.insert(0, q);
        self.recent_searches.truncate(12);
        let _ = self.save();
    }

    /// Contexto para montar o feed quando nao ha busca na sessao atual.
    pub fn feed_context(&self) -> String {
        if !self.last_search.trim().is_empty() {
            return self.last_search.trim().to_string();
        }
        self.interest_keywords(3).join(" ")
    }

    /// Ultimas faixas ouvidas — aparece na hora, sem rede.
    pub fn continue_listening(&self, limit: usize) -> Vec<Video> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        if let Some(last) = &self.last_music {
            if seen.insert(last.id.clone()) {
                out.push(last.clone());
            }
        }
        for entry in &self.recent_music {
            if out.len() >= limit {
                break;
            }
            if seen.insert(entry.video.id.clone()) {
                out.push(entry.video.clone());
            }
        }
        out
    }

    pub fn music_seed(&self) -> Option<Video> {
        self.last_music.clone()
    }

    pub fn top_music(&self, limit: usize) -> Vec<Video> {
        let mut entries = self.recent_music.clone();
        entries.sort_by(|a, b| {
            b.play_count
                .cmp(&a.play_count)
                .then(b.watched_at.cmp(&a.watched_at))
        });
        entries
            .into_iter()
            .take(limit)
            .map(|e| e.video)
            .collect()
    }

    pub fn played_ids(&self) -> std::collections::HashSet<String> {
        self.recent_music
            .iter()
            .map(|e| e.video.id.clone())
            .collect()
    }

    pub fn known_uploaders(&self) -> std::collections::HashSet<String> {
        self.recent_music
            .iter()
            .map(|e| crate::discover::normalize_uploader(&e.video.uploader))
            .filter(|u| u.len() > 2)
            .collect()
    }

    /// Ouvinte escuta predominantemente musica brasileira?
    pub fn prefers_brazilian(&self) -> bool {
        if self.recent_music.is_empty() {
            return false;
        }
        const BR: &[&str] = &[
            "sertanejo", "pagode", "forro", "forró", "mpb", "funk", "axe", "axé", "brasil",
            "brazil", "brega", "arrocha", "modao", "modão",
        ];
        let mut hits = 0u32;
        let total = self.recent_music.len().min(20) as u32;
        for entry in self.recent_music.iter().take(20) {
            let blob = format!("{} {}", entry.video.title, entry.video.uploader).to_lowercase();
            if BR.iter().any(|s| blob.contains(s)) {
                hits += 1;
            }
        }
        hits as f32 / total as f32 > 0.35
    }

    /// Palavras-tema extraidas do que o usuario mais escuta.
    pub fn interest_keywords(&self, limit: usize) -> Vec<String> {
        const STOP: &[&str] = &[
            "video", "videos", "oficial", "completo", "parte", "full", "the", "and", "for",
            "com", "para", "sobre", "como", "que", "uma", "por", "dos", "das", "nos", "nas",
            "this", "from", "with", "your", "what", "when", "where", "why", "who",
        ];
        let mut counts: HashMap<String, u32> = HashMap::new();
        for entry in self.recent_music.iter().take(24) {
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

fn history_path() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("promptub").join("history.json")
}

pub fn hydrate_state(state: &crate::state::AppState, history: &WatchHistory) {
    if state.last_video.lock().is_none() {
        if let Some(v) = history.last_music.clone() {
            *state.last_video.lock() = Some(v);
        }
    }
    if state.last_search().trim().is_empty() && !history.last_search.trim().is_empty() {
        state.set_last_search(history.last_search.clone());
    }
}

fn is_probably_video_id(s: &str) -> bool {
    s.len() == 11 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}
