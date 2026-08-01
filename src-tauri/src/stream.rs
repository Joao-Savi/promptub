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
pub const PREWARM_AHEAD: usize = 4;

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

    pub fn remove(&self, video_id: &str) {
        self.entries.lock().remove(video_id);
    }

    pub fn prewarm_status(&self) -> (usize, usize) {
        (
            self.prewarm_done.load(Ordering::Relaxed),
            self.prewarm_total.load(Ordering::Relaxed),
        )
    }

    pub fn prewarm_async(
        &self,
        cookies: String,
        items: Vec<Video>,
        audio_only: bool,
        quality: String,
    ) {
        let batch: Vec<Video> = items.into_iter().take(PREWARM_AHEAD).collect();
        if batch.is_empty() {
            return;
        }
        self.prewarm_total.store(batch.len(), Ordering::Relaxed);
        self.prewarm_done.store(0, Ordering::Relaxed);

        let entries = Arc::clone(&self.entries);
        let prewarm_done = Arc::clone(&self.prewarm_done);

        thread::spawn(move || {
            for (i, video) in batch.iter().enumerate() {
                if entries.lock().contains_key(&video.id) {
                    prewarm_done.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                let result = if audio_only {
                    resolve_stream_url(&cookies, video, true)
                } else {
                    resolve_stream_url_with_quality(&cookies, video, false, &quality)
                };
                if let Ok(url) = result {
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

pub fn resolve_stream_url(
    cookies: &str,
    video: &Video,
    audio_only: bool,
) -> Result<String, String> {
    resolve_stream_url_with_quality(cookies, video, audio_only, "720")
}

pub fn resolve_stream_url_with_quality(
    cookies: &str,
    video: &Video,
    audio_only: bool,
    quality: &str,
) -> Result<String, String> {
    if audio_only {
        return run_ytdlp_stream(cookies, video, true, quality, "youtube:player_client=android");
    }

    let format = video_format_string(quality);

    // ios + android em paralelo — primeiro que responder ganha.
    if let Ok(url) = race_ytdlp_clients(
        cookies,
        video,
        &format,
        &[
            "youtube:player_client=ios",
            "youtube:player_client=android",
        ],
    ) {
        return Ok(url);
    }

    for (fmt, client) in [
        ("22/18", "youtube:player_client=ios"),
        ("18", "youtube:player_client=android"),
    ] {
        if let Ok(url) = run_ytdlp_stream(cookies, video, false, fmt, client) {
            return Ok(url);
        }
    }

    run_ytdlp_stream(
        cookies,
        video,
        false,
        &format,
        "youtube:player_client=web,default",
    )
}

fn race_ytdlp_clients(
    cookies: &str,
    video: &Video,
    format: &str,
    clients: &[&str],
) -> Result<String, String> {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();
    let mut handles = Vec::with_capacity(clients.len());

    for client in clients {
        let tx = tx.clone();
        let cookies = cookies.to_string();
        let video = video.clone();
        let format = format.to_string();
        let client = client.to_string();
        handles.push(thread::spawn(move || {
            if let Ok(url) = run_ytdlp_stream(&cookies, &video, false, &format, &client) {
                let _ = tx.send(url);
            }
        }));
    }
    drop(tx);

    let deadline = Instant::now() + Duration::from_secs(18);
    loop {
        let wait = deadline
            .saturating_duration_since(Instant::now())
            .min(Duration::from_millis(250));
        if wait.is_zero() {
            break;
        }
        match rx.recv_timeout(wait) {
            Ok(url) => return Ok(url),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    for h in handles {
        let _ = h.join();
    }
    Err("stream nao encontrado".into())
}

fn video_format_string(quality: &str) -> String {
    let height = match quality {
        "360" => 360,
        "480" => 480,
        "1080" => 1080,
        "best" => 99999,
        _ => 720,
    };
    format!(
        "best[height<={height}][ext=mp4][acodec!=none][vcodec!=none][protocol^=http]/\
         best[height<={height}][acodec!=none][vcodec!=none][protocol^=http]/\
         best[ext=mp4][acodec!=none][vcodec!=none][protocol^=http]/\
         22/18"
    )
}

fn run_ytdlp_stream(
    cookies: &str,
    video: &Video,
    audio_only: bool,
    format: &str,
    extractor_args: &str,
) -> Result<String, String> {
    let ytdlp = find_ytdlp().ok_or("yt-dlp nao encontrado")?;
    let format = if audio_only {
        "bestaudio[ext=m4a]/bestaudio/best".to_string()
    } else {
        format.to_string()
    };

    let mut args = vec![
        "--quiet".into(),
        "--no-warnings".into(),
        "--no-playlist".into(),
        "--encoding".into(),
        "utf-8".into(),
        "-f".into(),
        format,
        "-g".into(),
    ];
    if !cookies.is_empty() {
        args.push("--cookies".into());
        args.push(cookies.into());
    }
    args.push("--extractor-args".into());
    args.push(extractor_args.into());
    args.push(video.url.clone());

    let output = utf8_cmd(&ytdlp)
        .args(&args)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(decode_bytes(&output.stderr).trim().to_string());
    }
    pick_stream_url(&decode_bytes(&output.stdout), audio_only)
}

fn pick_stream_url(stdout: &str, audio_only: bool) -> Result<String, String> {
    let urls: Vec<&str> = stdout
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("http"))
        .collect();
    if urls.is_empty() {
        return Err("URL de stream nao encontrada".into());
    }
    if urls.len() == 1 || audio_only {
        return Ok(urls[0].to_string());
    }
    // Progressive muxed de preferencia; se vier 2 linhas (video+audio DASH), falha clara.
    Err("stream separado DASH — mude qualidade ou tente outro video".into())
}

pub fn prewarm_queue_ahead(state: &crate::state::SharedState) {
    let audio_only = *state.audio_only.lock();
    let upcoming = state.queue.lock().upcoming_from_current(PREWARM_AHEAD);
    if upcoming.is_empty() {
        return;
    }
    let quality = state.video_quality.lock().clone();
    state
        .stream_cache
        .prewarm_async(state.cookies(), upcoming, audio_only, quality);
}
