use crate::deps::{find_ytdlp, utf8_cmd};
use crate::text::decode_bytes;
use crate::youtube::Video;
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const CACHE_TTL: Duration = Duration::from_secs(45 * 60);
const PREWARM_MAX: usize = 6;

struct CachedStream {
    url: String,
    fetched: Instant,
}

pub struct StreamCache {
    entries: Arc<Mutex<HashMap<String, CachedStream>>>,
    prewarm_total: Arc<AtomicUsize>,
    prewarm_done: Arc<AtomicUsize>,
}

impl StreamCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            prewarm_total: Arc::new(AtomicUsize::new(0)),
            prewarm_done: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn get(&self, video_id: &str) -> Option<String> {
        let map = self.entries.lock();
        let entry = map.get(video_id)?;
        if entry.fetched.elapsed() > CACHE_TTL {
            return None;
        }
        Some(entry.url.clone())
    }

    pub fn put(&self, video_id: String, url: String) {
        self.entries.lock().insert(
            video_id,
            CachedStream {
                url,
                fetched: Instant::now(),
            },
        );
    }

    pub fn prewarm_status(&self) -> (usize, usize) {
        (
            self.prewarm_done.load(Ordering::Relaxed),
            self.prewarm_total.load(Ordering::Relaxed),
        )
    }

    pub fn prewarm_async(&self, cookies: String, items: Vec<Video>, audio_only: bool) {
        let batch: Vec<Video> = items.into_iter().take(PREWARM_MAX).collect();
        if batch.is_empty() {
            return;
        }
        self.prewarm_total
            .store(batch.len(), Ordering::Relaxed);
        self.prewarm_done.store(0, Ordering::Relaxed);

        let entries = Arc::clone(&self.entries);
        let prewarm_done = Arc::clone(&self.prewarm_done);

        thread::spawn(move || {
            for (i, video) in batch.iter().enumerate() {
                if entries.lock().contains_key(&video.id) {
                    prewarm_done.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                if let Ok(url) = resolve_stream_url(&cookies, video, audio_only) {
                    entries.lock().insert(
                        video.id.clone(),
                        CachedStream {
                            url,
                            fetched: Instant::now(),
                        },
                    );
                }
                prewarm_done.fetch_add(1, Ordering::Relaxed);
                if i + 1 < batch.len() {
                    thread::sleep(Duration::from_millis(120));
                }
            }
        });
    }
}

#[derive(Serialize)]
pub struct PrewarmStatus {
    pub done: usize,
    pub total: usize,
}

pub fn resolve_stream_url(
    cookies: &str,
    video: &Video,
    audio_only: bool,
) -> Result<String, String> {
    let ytdlp = find_ytdlp().ok_or("yt-dlp nao encontrado")?;
    let format = if audio_only {
        "bestaudio[ext=m4a]/bestaudio/best"
    } else if video.is_live {
        "best"
    } else {
        "best[height<=720][ext=mp4]/best[height<=720]/best"
    };

    let mut args = vec![
        "--quiet".into(),
        "--no-warnings".into(),
        "--no-playlist".into(),
        "--encoding".into(),
        "utf-8".into(),
        "-f".into(),
        format.into(),
        "-g".into(),
    ];
    if !cookies.is_empty() {
        args.push("--cookies".into());
        args.push(cookies.into());
    }
    if !audio_only {
        args.push("--extractor-args".into());
        args.push("youtube:player_client=web".into());
    } else {
        args.push("--extractor-args".into());
        args.push("youtube:player_client=android".into());
    }
    args.push(video.url.clone());

    let output = utf8_cmd(&ytdlp)
        .args(&args)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(decode_bytes(&output.stderr).trim().to_string());
    }
    let stdout = decode_bytes(&output.stdout);
    let url = stdout
        .lines()
        .find(|l| l.starts_with("http"))
        .unwrap_or("")
        .trim()
        .to_string();
    if url.is_empty() {
        return Err("URL de stream nao encontrada".into());
    }
    Ok(url)
}
