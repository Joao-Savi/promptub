use crate::deps::{find_ytdlp, utf8_cmd};
use crate::text::decode_bytes;
use crate::youtube::{self, Video};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CACHE_TTL: Duration = Duration::from_secs(45 * 60);
const DISK_CACHE_TTL_SECS: u64 = 45 * 60;
const DISK_CACHE_MAX: usize = 48;
pub const PREWARM_AHEAD: usize = 3;

struct CachedStream {
    url: String,
    fetched: Instant,
}

#[derive(Serialize, Deserialize)]
struct DiskStreamEntry {
    url: String,
    saved_at: u64,
}

pub struct StreamCache {
    entries: Arc<Mutex<HashMap<String, CachedStream>>>,
    prewarm_total: Arc<AtomicUsize>,
    prewarm_done: Arc<AtomicUsize>,
}

impl StreamCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(load_disk_cache())),
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
        if !youtube::is_allowed_stream_url(&entry.url) {
            return None;
        }
        Some(entry.url.clone())
    }

    pub fn put(&self, video_id: String, url: String) {
        if !youtube::is_allowed_stream_url(&url) {
            return;
        }
        self.entries.lock().insert(
            video_id,
            CachedStream {
                url: url.clone(),
                fetched: Instant::now(),
            },
        );
        save_disk_cache(&self.entries.lock());
    }

    pub fn prewarm_status(&self) -> (usize, usize) {
        (
            self.prewarm_done.load(Ordering::Relaxed),
            self.prewarm_total.load(Ordering::Relaxed),
        )
    }

    pub fn prewarm_async(&self, cookies: String, items: Vec<Video>) {
        let batch: Vec<Video> = items.into_iter().take(PREWARM_AHEAD).collect();
        if batch.is_empty() {
            return;
        }
        self.prewarm_total.store(batch.len(), Ordering::Relaxed);
        self.prewarm_done.store(0, Ordering::Relaxed);

        let entries = Arc::clone(&self.entries);
        let prewarm_done = Arc::clone(&self.prewarm_done);

        thread::spawn(move || {
            for (i, track) in batch.iter().enumerate() {
                if entries.lock().contains_key(&track.id) {
                    prewarm_done.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                if let Ok(url) = resolve_stream_url(&cookies, track) {
                    entries.lock().insert(
                        track.id.clone(),
                        CachedStream {
                            url,
                            fetched: Instant::now(),
                        },
                    );
                    save_disk_cache(&entries.lock());
                }
                prewarm_done.fetch_add(1, Ordering::Relaxed);
                if i + 1 < batch.len() {
                    thread::sleep(Duration::from_millis(80));
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

pub fn resolve_stream_url(cookies: &str, track: &Video) -> Result<String, String> {
    run_ytdlp_stream(cookies, track, "youtube:player_client=android")
}

fn run_ytdlp_stream(cookies: &str, track: &Video, extractor_args: &str) -> Result<String, String> {
    let ytdlp = find_ytdlp().ok_or("yt-dlp nao encontrado")?;
    let mut args = vec![
        "--quiet".into(),
        "--no-warnings".into(),
        "--no-playlist".into(),
        "--encoding".into(),
        "utf-8".into(),
        "-f".into(),
        "bestaudio[ext=m4a]/bestaudio/best".into(),
        "-g".into(),
    ];
    if !cookies.is_empty() {
        args.push("--cookies".into());
        args.push(cookies.into());
    }
    args.push("--extractor-args".into());
    args.push(extractor_args.into());
    args.push(track.url.clone());

    let output = utf8_cmd(&ytdlp)
        .args(&args)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(decode_bytes(&output.stderr).trim().to_string());
    }
    pick_stream_url(&decode_bytes(&output.stdout))
}

fn pick_stream_url(stdout: &str) -> Result<String, String> {
    for line in stdout.lines().map(str::trim) {
        if youtube::is_allowed_stream_url(line) {
            return Ok(line.to_string());
        }
    }
    Err("URL de stream nao encontrada ou dominio nao permitido".into())
}

pub fn prewarm_queue_ahead(state: &crate::state::SharedState) {
    let upcoming = state.queue.lock().upcoming_from_current(PREWARM_AHEAD);
    if upcoming.is_empty() {
        return;
    }
    state
        .stream_cache
        .prewarm_async(state.cookies(), upcoming);
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn stream_cache_path() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("promptub").join("stream_cache.json")
}

fn load_disk_cache() -> HashMap<String, CachedStream> {
    let path = stream_cache_path();
    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return HashMap::new(),
    };
    let disk: HashMap<String, DiskStreamEntry> = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };
    let now = now_secs();
    let mut out = HashMap::new();
    for (id, entry) in disk {
        if now.saturating_sub(entry.saved_at) > DISK_CACHE_TTL_SECS {
            continue;
        }
        if !youtube::is_allowed_stream_url(&entry.url) {
            continue;
        }
        out.insert(
            id,
            CachedStream {
                url: entry.url,
                fetched: Instant::now()
                    - Duration::from_secs(now.saturating_sub(entry.saved_at)),
            },
        );
    }
    out
}

fn save_disk_cache(map: &HashMap<String, CachedStream>) {
    let path = stream_cache_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let now = now_secs();
    let mut disk: HashMap<String, DiskStreamEntry> = HashMap::new();
    for (id, entry) in map.iter() {
        if entry.fetched.elapsed() > CACHE_TTL {
            continue;
        }
        let saved_at = now.saturating_sub(entry.fetched.elapsed().as_secs());
        disk.insert(
            id.clone(),
            DiskStreamEntry {
                url: entry.url.clone(),
                saved_at,
            },
        );
    }
    let mut entries: Vec<_> = disk.into_iter().collect();
    entries.sort_by(|a, b| b.1.saved_at.cmp(&a.1.saved_at));
    entries.truncate(DISK_CACHE_MAX);
    let trimmed: HashMap<String, DiskStreamEntry> = entries.into_iter().collect();
    if let Ok(json) = serde_json::to_string(&trimmed) {
        let _ = fs::write(path, json);
    }
}
