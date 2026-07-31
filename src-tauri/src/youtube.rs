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
pub const HOME_REC_LIMIT: usize = 8;
pub const HOME_LIVE_LIMIT: usize = 4;

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
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    let mut args = ytdlp_base(cookies);
    args.extend([
        "--flat-playlist".into(),
        "--print".into(),
        PRINT_FIELDS.into(),
        format!("ytsearch{limit}:{q}"),
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

#[derive(Serialize)]
pub struct HomeFeed {
    pub recommended: Vec<Video>,
    pub live: Vec<Video>,
    pub seed_label: String,
}

fn push_unique(out: &mut Vec<Video>, seen: &mut HashSet<String>, items: Vec<Video>) {
    for v in items {
        if seen.insert(v.id.clone()) {
            out.push(v);
        }
    }
}

pub(crate) fn build_home_feed(
    cookies: &str,
    mode: &str,
    seed: Option<Video>,
) -> Result<HomeFeed, String> {
    let is_music = mode != "video";
    let mut seen = HashSet::new();
    let mut recommended = Vec::new();

    let seed_label = if let Some(v) = seed {
        let label = v.title.clone();
        if is_music {
            push_unique(
                &mut recommended,
                &mut seen,
                fetch_mix(cookies, &v.id, HOME_REC_LIMIT)?,
            );
        } else {
            push_unique(
                &mut recommended,
                &mut seen,
                fetch_rd(cookies, &v.id, HOME_REC_LIMIT)?,
            );
        }
        label
    } else if is_music {
        push_unique(
            &mut recommended,
            &mut seen,
            fetch_search(cookies, "music mix", HOME_REC_LIMIT)?,
        );
        "explorar musicas".into()
    } else {
        push_unique(
            &mut recommended,
            &mut seen,
            fetch_search(cookies, "videos em alta", HOME_REC_LIMIT)?,
        );
        "explorar videos".into()
    };

    recommended.truncate(HOME_REC_LIMIT);

    let live = if is_music {
        vec![]
    } else {
        fetch_live(cookies, HOME_LIVE_LIMIT).unwrap_or_default()
    };

    Ok(HomeFeed {
        recommended,
        live,
        seed_label,
    })
}

#[tauri::command]
pub fn search(state: State<'_, SharedState>, query: String) -> Result<Vec<Video>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }
    state.set_last_search(q.to_string());
    fetch_search(&state.cookies(), q, SEARCH_LIMIT)
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

#[tauri::command]
pub fn home_recommendations(
    state: State<'_, SharedState>,
    mode: String,
) -> Result<HomeFeed, String> {
    let seed = if mode == "video" {
        state
            .last_watch_video
            .lock()
            .clone()
            .or_else(|| state.last_video.lock().clone())
    } else {
        state
            .last_music_video
            .lock()
            .clone()
            .or_else(|| state.last_video.lock().clone())
    };
    build_home_feed(&state.cookies(), &mode, seed)
}
