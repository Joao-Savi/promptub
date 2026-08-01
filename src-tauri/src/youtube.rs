use crate::deps::{find_ytdlp, utf8_cmd};
use crate::text::{decode_bytes, repair_mojibake};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub uploader: String,
    pub duration: String,
    pub url: String,
    pub thumbnail: String,
    #[serde(default)]
    pub is_live: bool,
}

const PRINT_FIELDS: &str = "%(id)s\t%(title)s\t%(uploader)s\t%(duration_string)s\t%(live_status)s";

pub const SEARCH_LIMIT: usize = 10;
pub const RELATED_LIMIT: usize = 6;
pub const HOME_FEED_LIMIT: usize = 12;
pub const HOME_REC_LIMIT: usize = 10;
pub const HOME_LIVE_LIMIT: usize = 4;
pub const HOME_CHANNEL_LIMIT: usize = 12;
pub const VIDEO_CONTEXT_FEED_LIMIT: usize = 24;

impl Video {
    pub(crate) fn from_line(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 {
            return None;
        }
        let id = parts[0].trim().to_string();
        if id.is_empty() {
            return None;
        }
        let live_status = parts.get(4).unwrap_or(&"").trim();
        let is_live = live_status == "is_live";
        let duration_raw = parts.get(3).unwrap_or(&"").trim();
        let duration = if is_live {
            "LIVE".into()
        } else if live_status == "was_live" && duration_raw.is_empty() {
            "GRAVACAO".into()
        } else {
            duration_raw.to_string()
        };
        Some(Self {
            title: repair_mojibake(parts[1].trim()),
            uploader: repair_mojibake(parts.get(2).unwrap_or(&"").trim()),
            duration,
            url: format!("https://www.youtube.com/watch?v={id}"),
            thumbnail: format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"),
            id,
            is_live,
        })
    }
}

fn ytdlp_base(cookies: &str) -> Vec<String> {
    let mut args = vec![
        "--quiet".into(),
        "--no-warnings".into(),
        "--no-progress".into(),
        "--encoding".into(),
        "utf-8".into(),
    ];
    if !cookies.is_empty() {
        args.push("--cookies".into());
        args.push(cookies.into());
    }
    args
}

fn yt_extractor_args(cookies: &str) -> &'static str {
    if cookies.is_empty() {
        "youtube:player_client=android,web"
    } else {
        "youtube:player_client=web,default"
    }
}

fn push_yt_extractor(args: &mut Vec<String>, cookies: &str) {
    args.push("--extractor-args".into());
    args.push(yt_extractor_args(cookies).into());
}

pub(crate) fn fetch_subscriptions(cookies: &str, limit: usize) -> Result<Vec<Video>, String> {
    if cookies.is_empty() {
        return Ok(vec![]);
    }
    let mut args = ytdlp_base(cookies);
    push_yt_extractor(&mut args, cookies);
    args.extend([
        "--flat-playlist".into(),
        "--playlist-end".into(),
        limit.to_string(),
        "--print".into(),
        PRINT_FIELDS.into(),
        "https://www.youtube.com/feed/subscriptions".into(),
    ]);
    run_list(args)
}

pub(crate) fn fetch_yt_history(cookies: &str, limit: usize) -> Result<Vec<Video>, String> {
    if cookies.is_empty() {
        return Ok(vec![]);
    }
    let mut args = ytdlp_base(cookies);
    push_yt_extractor(&mut args, cookies);
    args.extend([
        "--flat-playlist".into(),
        "--playlist-end".into(),
        limit.to_string(),
        "--print".into(),
        PRINT_FIELDS.into(),
        "https://www.youtube.com/feed/history".into(),
    ]);
    run_list(args)
}

pub(crate) fn run_list(args: Vec<String>) -> Result<Vec<Video>, String> {
    let ytdlp = find_ytdlp().ok_or("yt-dlp nao encontrado")?;
    let output = utf8_cmd(&ytdlp)
        .args(&args)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(decode_bytes(&output.stderr).trim().to_string());
    }
    Ok(decode_bytes(&output.stdout)
        .lines()
        .filter_map(Video::from_line)
        .collect())
}

pub(crate) fn fetch_search(cookies: &str, query: &str, limit: usize) -> Result<Vec<Video>, String> {
    search_with_mode(cookies, query, limit, false)
}

/// Busca ordenada por data (videos mais recentes primeiro) — proxy de "novidades".
pub(crate) fn fetch_search_recent(cookies: &str, query: &str, limit: usize) -> Result<Vec<Video>, String> {
    search_with_mode(cookies, query, limit, true)
}

fn search_with_mode(cookies: &str, query: &str, limit: usize, by_date: bool) -> Result<Vec<Video>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let mut args = ytdlp_base(cookies);
    push_yt_extractor(&mut args, cookies);
    args.extend([
        "--flat-playlist".into(),
        "--print".into(),
        PRINT_FIELDS.into(),
        if by_date {
            format!("ytsearchdate{limit}:{q}")
        } else {
            format!("ytsearch{limit}:{q}")
        },
    ]);
    run_list(args)
}

pub(crate) fn fetch_mix(cookies: &str, video_id: &str, limit: usize) -> Result<Vec<Video>, String> {
    if video_id.is_empty() {
        return Ok(vec![]);
    }
    let mix = format!("https://www.youtube.com/watch?v={video_id}&list=RDMM{video_id}");
    let mut args = ytdlp_base(cookies);
    args.extend([
        "--flat-playlist".into(),
        "--playlist-end".into(),
        limit.to_string(),
        "--print".into(),
        PRINT_FIELDS.into(),
        mix,
    ]);
    let mut items = run_list(args)?;
    items.retain(|v| v.id != video_id);
    Ok(items)
}

pub(crate) fn fetch_rd(cookies: &str, video_id: &str, limit: usize) -> Result<Vec<Video>, String> {
    if video_id.is_empty() {
        return Ok(vec![]);
    }
    let radio = format!("https://www.youtube.com/watch?v={video_id}&list=RD{video_id}");
    let mut args = ytdlp_base(cookies);
    args.extend([
        "--flat-playlist".into(),
        "--playlist-end".into(),
        limit.to_string(),
        "--print".into(),
        PRINT_FIELDS.into(),
        radio,
    ]);
    let mut items = run_list(args)?;
    items.retain(|v| v.id != video_id);
    Ok(items)
}

pub(crate) fn fetch_live(cookies: &str, limit: usize) -> Result<Vec<Video>, String> {
    let mut args = ytdlp_base(cookies);
    args.extend([
        "--flat-playlist".into(),
        "--print".into(),
        PRINT_FIELDS.into(),
        format!("ytsearch{}:live", limit.saturating_mul(2)),
    ]);
    let mut items = run_list(args)?;
    items.retain(|v| v.is_live);
    items.truncate(limit);
    Ok(items)
}

fn fetch_live_search(cookies: &str, query: &str, limit: usize) -> Result<Vec<Video>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let mut items = fetch_search(cookies, &format!("{q} ao vivo"), limit.saturating_mul(3))?;
    items.retain(|v| v.is_live);
    if items.len() < limit {
        let mut more = fetch_search(cookies, q, limit.saturating_mul(2))?;
        more.retain(|v| v.is_live);
        items.append(&mut more);
    }
    items.truncate(limit);
    Ok(items)
}

pub(crate) fn fetch_video(cookies: &str, video_id: &str) -> Result<Option<Video>, String> {
    if video_id.is_empty() {
        return Ok(None);
    }
    let url = format!("https://www.youtube.com/watch?v={video_id}");
    let mut args = ytdlp_base(cookies);
    args.extend([
        "--no-playlist".into(),
        "--print".into(),
        PRINT_FIELDS.into(),
        url,
    ]);
    Ok(run_list(args)?.into_iter().next())
}

use crate::state::SharedState;
use std::collections::HashSet;
use tauri::State;

fn push_unique(out: &mut Vec<Video>, seen: &mut HashSet<String>, items: Vec<Video>) {
    for v in items {
        if seen.insert(v.id.clone()) {
            out.push(v);
        }
    }
}

#[derive(Serialize)]
pub struct HomeFeed {
    pub feed: Vec<Video>,
    pub recommended: Vec<Video>,
    pub live: Vec<Video>,
    pub channel_news: Vec<Video>,
    pub seed_label: String,
}

fn build_music_home(cookies: &str, seed: Option<Video>) -> Result<HomeFeed, String> {
    let mut seen = HashSet::new();
    let mut recommended = Vec::new();

    let seed_label = if let Some(v) = seed {
        let label = v.title.clone();
        push_unique(
            &mut recommended,
            &mut seen,
            fetch_mix(cookies, &v.id, HOME_REC_LIMIT)?,
        );
        label
    } else {
        push_unique(
            &mut recommended,
            &mut seen,
            fetch_search(cookies, "music mix", HOME_REC_LIMIT)?,
        );
        "explorar musicas".into()
    };

    recommended.truncate(HOME_REC_LIMIT);

    Ok(HomeFeed {
        feed: vec![],
        recommended,
        live: vec![],
        channel_news: vec![],
        seed_label,
    })
}

fn build_video_home(
    cookies: &str,
    seed: Option<Video>,
    history: &crate::history::WatchHistory,
) -> Result<HomeFeed, String> {
    let rotation = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as usize)
        .unwrap_or(0);

    let (feed, recommended, live, channel_news, seed_label) =
        crate::video_recommend::recommend_home_feed(cookies, seed, history, rotation)?;

    Ok(HomeFeed {
        feed,
        recommended,
        live,
        channel_news,
        seed_label,
    })
}

pub(crate) fn build_home_feed(
    cookies: &str,
    mode: &str,
    seed: Option<Video>,
    history: &crate::history::WatchHistory,
) -> Result<HomeFeed, String> {
    if mode == "video" {
        build_video_home(cookies, seed, history)
    } else {
        build_music_home(cookies, seed.or_else(|| history.music_seed()))
    }
}

pub fn parse_youtube_id(input: &str) -> Option<String> {
    let s = input.trim();
    if s.len() == 11 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Some(s.to_string());
    }
    for prefix in [
        "https://www.youtube.com/watch?v=",
        "http://www.youtube.com/watch?v=",
        "https://youtube.com/watch?v=",
        "https://youtu.be/",
        "http://youtu.be/",
        "https://www.youtube.com/embed/",
        "https://www.youtube.com/live/",
        "https://www.youtube.com/shorts/",
    ] {
        if let Some(rest) = s.strip_prefix(prefix) {
            let id: String = rest
                .chars()
                .take_while(|c| *c != '&' && *c != '?' && *c != '/')
                .collect();
            if id.len() == 11 {
                return Some(id);
            }
        }
    }
    if let Some(idx) = s.find("v=") {
        let id: String = s[idx + 2..]
            .chars()
            .take_while(|c| *c != '&' && *c != '#')
            .collect();
        if id.len() == 11 {
            return Some(id);
        }
    }
    None
}

#[tauri::command]
pub async fn resolve_video(
    state: State<'_, SharedState>,
    video_id: String,
) -> Result<Video, String> {
    let id = parse_youtube_id(&video_id).ok_or("ID ou URL do YouTube invalido")?;
    state.set_last_search(id.clone());
    let cookies = state.cookies();
    tauri::async_runtime::spawn_blocking(move || {
        fetch_video(&cookies, &id)?.ok_or_else(|| "Video nao encontrado".into())
    })
    .await
    .map_err(|e| format!("resolve: {e}"))?
}

#[tauri::command]
pub async fn search(state: State<'_, SharedState>, query: String) -> Result<Vec<Video>, String> {
    let q = query.trim().to_string();
    if q.is_empty() {
        return Ok(vec![]);
    }
    state.set_last_search(q.clone());
    let cookies = state.cookies();
    tauri::async_runtime::spawn_blocking(move || fetch_search(&cookies, &q, SEARCH_LIMIT))
        .await
        .map_err(|e| format!("search: {e}"))?
}

#[tauri::command]
pub fn related(state: State<'_, SharedState>, video_id: String) -> Result<Vec<Video>, String> {
    let cookies = state.cookies();
    let audio_only = *state.audio_only.lock();
    let items = if audio_only {
        fetch_mix(&cookies, &video_id, RELATED_LIMIT)?
    } else {
        fetch_rd(&cookies, &video_id, RELATED_LIMIT)?
    };
    Ok(items)
}

fn fetch_video_context_feed(cookies: &str, video: &Video) -> Result<Vec<Video>, String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    push_unique(&mut out, &mut seen, fetch_rd(cookies, &video.id, 16)?);

    let theme = crate::discover::simplify_for_search(&video.title);
    if theme.len() >= 4 {
        push_unique(
            &mut out,
            &mut seen,
            fetch_search(cookies, &theme, 10)?,
        );
    }

    let artist = crate::discover::normalize_uploader(&video.uploader);
    if artist.len() > 3 {
        push_unique(
            &mut out,
            &mut seen,
            fetch_search(cookies, &artist, 8)?,
        );
    }

    out.retain(|v| v.id != video.id);
    out.truncate(VIDEO_CONTEXT_FEED_LIMIT);
    Ok(out)
}

#[tauri::command]
pub async fn video_context_feed(
    state: State<'_, SharedState>,
    video_id: String,
) -> Result<Vec<Video>, String> {
    let id = parse_youtube_id(&video_id).unwrap_or_else(|| video_id.trim().to_string());
    if id.is_empty() {
        return Ok(vec![]);
    }
    let cookies = state.cookies();
    tauri::async_runtime::spawn_blocking(move || {
        let video = fetch_video(&cookies, &id)?.ok_or("Video nao encontrado")?;
        fetch_video_context_feed(&cookies, &video)
    })
    .await
    .map_err(|e| format!("video feed: {e}"))?
}

#[tauri::command]
pub async fn home_recommendations(
    state: State<'_, SharedState>,
    mode: String,
) -> Result<HomeFeed, String> {
    let history = state.watch_history.lock().clone();
    let seed = if mode == "video" {
        state
            .last_watch_video
            .lock()
            .clone()
            .or_else(|| state.last_video.lock().clone())
            .or_else(|| history.video_seed())
    } else {
        state
            .last_music_video
            .lock()
            .clone()
            .or_else(|| state.last_video.lock().clone())
            .or_else(|| history.music_seed())
    };
    let cookies = state.cookies();
    tauri::async_runtime::spawn_blocking(move || build_home_feed(&cookies, &mode, seed, &history))
        .await
        .map_err(|e| format!("recommendations: {e}"))?
}
