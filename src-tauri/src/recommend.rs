use crate::state::SharedState;
use crate::youtube::{self, Video};
use serde::Serialize;
use std::collections::HashSet;
use tauri::State;

const PLAYLIST_TARGET: usize = 25;

#[derive(Serialize)]
pub struct RecommendedPlaylist {
    pub items: Vec<Video>,
    pub seed_label: String,
    pub count: usize,
}

#[tauri::command]
pub fn recommended_playlist(
    state: State<'_, SharedState>,
    seed_video_id: Option<String>,
    seed_query: Option<String>,
) -> Result<RecommendedPlaylist, String> {
    let cookies = state.cookies();
    let seed_id = seed_video_id
        .filter(|s| !s.is_empty())
        .or_else(|| state.last_video_id())
        .or_else(|| state.queue.lock().current_video().map(|v| v.id));

    let fallback_query = state.last_search();
    let query_ref = seed_query.as_deref().or_else(|| {
        if fallback_query.is_empty() {
            None
        } else {
            Some(fallback_query.as_str())
        }
    });

    let (items, seed_label) = build_playlist(&cookies, seed_id.as_deref(), query_ref)?;

    if items.is_empty() {
        return Err("Nao foi possivel montar a playlist. Toque ou busque algo antes.".into());
    }

    let count = items.len();
    Ok(RecommendedPlaylist {
        items,
        seed_label,
        count,
    })
}

fn build_playlist(
    cookies: &str,
    seed_video_id: Option<&str>,
    seed_query: Option<&str>,
) -> Result<(Vec<Video>, String), String> {
    let mut seen = HashSet::new();
    let mut playlist = Vec::new();

    let seed = resolve_seed(cookies, seed_video_id, seed_query)?;

    let seed_label = match &seed {
        Some(v) => v.title.clone(),
        None => seed_query.unwrap_or("Para voce").to_string(),
    };

    if let Some(seed_video) = seed {
        push_unique(&mut playlist, &mut seen, seed_video);

        let seed_id = playlist[0].id.clone();
        push_many(
            &mut playlist,
            &mut seen,
            youtube::fetch_mix(cookies, &seed_id, 20)?,
        );

        if !playlist[0].uploader.is_empty() {
            let artist_query = format!(
                "{} {}",
                playlist[0].uploader,
                simplify_for_search(&playlist[0].title)
            );
            push_many(
                &mut playlist,
                &mut seen,
                youtube::fetch_search(cookies, &artist_query, 10)?,
            );
        }

        let title_query = simplify_for_search(&playlist[0].title);
        if title_query.len() >= 4 {
            push_many(
                &mut playlist,
                &mut seen,
                youtube::fetch_search(cookies, &title_query, 8)?,
            );
        }

        let anchors: Vec<String> = playlist.iter().take(4).map(|v| v.id.clone()).collect();
        for anchor_id in anchors {
            if playlist.len() >= PLAYLIST_TARGET {
                break;
            }
            push_many(
                &mut playlist,
                &mut seen,
                youtube::fetch_mix(cookies, &anchor_id, 6)?,
            );
        }
    } else if let Some(q) = seed_query {
        push_many(
            &mut playlist,
            &mut seen,
            youtube::fetch_search(cookies, q, 12)?,
        );
        if let Some(first) = playlist.first() {
            let first_id = first.id.clone();
            push_many(
                &mut playlist,
                &mut seen,
                youtube::fetch_mix(cookies, &first_id, 18)?,
            );
        }
    } else {
        return Err("Toque algo ou faca uma busca para gerar a playlist recomendada.".into());
    }

    playlist.truncate(PLAYLIST_TARGET);
    Ok((playlist, seed_label))
}

fn resolve_seed(
    cookies: &str,
    seed_video_id: Option<&str>,
    seed_query: Option<&str>,
) -> Result<Option<Video>, String> {
    if let Some(id) = seed_video_id.filter(|s| !s.is_empty()) {
        if let Some(v) = youtube::fetch_video(cookies, id)? {
            return Ok(Some(v));
        }
    }
    if let Some(q) = seed_query.filter(|s| !s.is_empty()) {
        if let Some(v) = youtube::fetch_search(cookies, q, 1)?.into_iter().next() {
            return Ok(Some(v));
        }
    }
    Ok(None)
}

fn push_unique(out: &mut Vec<Video>, seen: &mut HashSet<String>, video: Video) {
    if seen.insert(video.id.clone()) {
        out.push(video);
    }
}

fn push_many(out: &mut Vec<Video>, seen: &mut HashSet<String>, videos: Vec<Video>) {
    for v in videos {
        if out.len() >= PLAYLIST_TARGET {
            break;
        }
        push_unique(out, seen, v);
    }
}

fn simplify_for_search(title: &str) -> String {
    let lower = title.to_lowercase();
    let noise = [
        "official video",
        "official music video",
        "video oficial",
        "clipe oficial",
        "lyrics",
        "legendado",
        "ao vivo",
        "live",
        "hd",
        "4k",
        "ft.",
        "feat.",
    ];
    let mut s = lower;
    for n in noise {
        s = s.replace(n, " ");
    }
    s.split(|c: char| !c.is_alphanumeric() && c != ' ')
        .filter(|w| w.len() > 2)
        .take(6)
        .collect::<Vec<_>>()
        .join(" ")
}
