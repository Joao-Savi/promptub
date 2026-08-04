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
pub const PLAYLIST_MAX: usize = 100;

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

pub(crate) fn fetch_track(cookies: &str, video_id: &str) -> Result<Option<Video>, String> {
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

pub fn is_youtube_playlist_url(input: &str) -> bool {
    normalize_playlist_url(input).is_some()
}

pub fn normalize_playlist_url(input: &str) -> Option<String> {
    let s = input.trim();
    let lower = s.to_lowercase();
    if !lower.contains("list=") {
        return None;
    }
    if !lower.contains("youtube.com")
        && !lower.contains("youtu.be")
        && !lower.contains("music.youtube.com")
    {
        return None;
    }
    let list_id: String = s
        .split("list=")
        .nth(1)?
        .chars()
        .take_while(|c| *c != '&' && *c != '#')
        .collect();
    if list_id.len() < 10 {
        return None;
    }
    Some(format!(
        "https://www.youtube.com/playlist?list={list_id}"
    ))
}

pub fn fetch_playlist(cookies: &str, url: &str, limit: usize) -> Result<Vec<Video>, String> {
    let mut args = ytdlp_base(cookies);
    push_yt_extractor(&mut args, cookies);
    args.extend([
        "--flat-playlist".into(),
        "--playlist-end".into(),
        limit.to_string(),
        "--print".into(),
        PRINT_FIELDS.into(),
        url.to_string(),
    ]);
    let items = run_list(args)?;
    Ok(items
        .into_iter()
        .filter(|v| crate::discover::is_playable_track(v))
        .collect())
}

use crate::state::SharedState;
use tauri::State;

#[derive(Clone, Serialize, Deserialize)]
pub struct GenreFeedRow {
    pub label: String,
    pub items: Vec<Video>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HomeFeed {
    pub recommended: Vec<Video>,
    #[serde(default)]
    pub continue_listening: Vec<Video>,
    #[serde(default)]
    pub most_played: Vec<Video>,
    #[serde(default)]
    pub new_artists: Vec<Video>,
    #[serde(default)]
    pub history_mix: Vec<Video>,
    #[serde(default)]
    pub genre_rows: Vec<GenreFeedRow>,
    pub seed_label: String,
    pub feed: Vec<Video>,
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

pub(crate) fn is_youtube_watch_url(input: &str) -> bool {
    let s = input.trim();
    if s.len() == 11
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return true;
    }
    if !s.contains("youtube.com") && !s.contains("youtu.be") {
        return false;
    }
    parse_youtube_id(s).is_some()
}

pub(crate) fn is_allowed_stream_url(url: &str) -> bool {
    let url = url.trim();
    if !url.starts_with("https://") {
        return false;
    }
    let Some(host) = url_host(url) else {
        return false;
    };
    host == "googlevideo.com"
        || host.ends_with(".googlevideo.com")
        || host == "youtube.com"
        || host.ends_with(".youtube.com")
}

fn url_host(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let authority = rest.split('/').next()?;
    let host = authority.rsplit('@').next()?;
    Some(host.split(':').next()?.to_ascii_lowercase())
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
        fetch_track(&cookies, &id)?.ok_or_else(|| "Faixa nao encontrada".into())
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
    state.watch_history.lock().record_search(&q);
    let cookies = state.cookies();
    tauri::async_runtime::spawn_blocking(move || {
        let items = fetch_search(&cookies, &q, SEARCH_LIMIT)?;
        Ok(crate::discover::refine_search_results(&q, items))
    })
    .await
    .map_err(|e| format!("search: {e}"))?
}

#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn allows_googlevideo_stream() {
        assert!(is_allowed_stream_url(
            "https://rr1---sn-abc.googlevideo.com/videoplayback?expire=1"
        ));
    }

    #[test]
    fn rejects_non_stream_hosts() {
        assert!(!is_allowed_stream_url("https://evil.com/track.mp3"));
        assert!(!is_allowed_stream_url("http://rr1---sn-abc.googlevideo.com/videoplayback"));
    }

    #[test]
    fn accepts_youtube_watch_urls() {
        assert!(is_youtube_watch_url("dQw4w9WgXcQ"));
        assert!(is_youtube_watch_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ"));
        assert!(!is_youtube_watch_url("https://evil.com/watch?v=dQw4w9WgXcQ"));
    }

    #[test]
    fn normalizes_playlist_url() {
        let url = normalize_playlist_url(
            "https://www.youtube.com/watch?v=abc12345678&list=PLrAXtmRdnEQy6nuLMH",
        )
        .unwrap();
        assert!(url.contains("list=PLrAXtmRdnEQy6nuLMH"));
        assert!(is_youtube_playlist_url(
            "https://music.youtube.com/playlist?list=PLabcdefghijklmnop"
        ));
    }
}
