//! Feed progressivo — seções independentes para carregar em etapas.

use crate::history::WatchHistory;
use crate::music_recommend;
use crate::state::SharedState;
use crate::youtube::Video;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Clone, Serialize)]
pub struct FeedLocal {
    pub continue_listening: Vec<Video>,
    pub most_played: Vec<Video>,
    pub seed_label: String,
}

#[derive(Clone, Serialize)]
pub struct FeedSectionResult {
    pub section: String,
    pub items: Vec<Video>,
}

#[derive(Clone, Serialize)]
pub struct GenreRowsResult {
    pub rows: Vec<crate::youtube::GenreFeedRow>,
}

pub fn build_genre_rows(
    cookies: &str,
    history: &WatchHistory,
    last_search: &str,
    rotation: usize,
    exclude_ids: &[String],
) -> Result<Vec<crate::youtube::GenreFeedRow>, String> {
    let trends = history.genre_trends(3);
    if trends.len() < 2 {
        return Ok(vec![]);
    }

    let mut seen: HashSet<String> = exclude_ids.iter().cloned().collect();
    for id in history.played_ids() {
        seen.insert(id);
    }

    let mut rows = Vec::new();
    for trend in trends {
        let items = music_recommend::build_genre_trend_row(
            cookies,
            &trend,
            history,
            last_search,
            rotation,
            &mut seen,
        )?;
        if !items.is_empty() {
            rows.push(crate::youtube::GenreFeedRow {
                label: trend.label,
                items,
            });
        }
    }
    Ok(rows)
}

pub fn build_local_parts(history: &WatchHistory, last_search: &str, seed: Option<Video>) -> FeedLocal {
    FeedLocal {
        continue_listening: history.continue_listening(8),
        most_played: history.top_music(10),
        seed_label: music_recommend::seed_label(history, last_search, &seed),
    }
}

pub fn build_section(
    cookies: &str,
    history: &WatchHistory,
    seed: Option<Video>,
    last_search: &str,
    rotation: usize,
    section: &str,
    exclude_ids: &[String],
    essential: bool,
) -> Result<Vec<Video>, String> {
    let seed = seed.or_else(|| history.music_seed());
    let prefer_br = history.prefers_brazilian();
    let mut seen: HashSet<String> = exclude_ids.iter().cloned().collect();
    for id in history.played_ids() {
        seen.insert(id);
    }
    for id in history.blocked_ids() {
        seen.insert(id);
    }

    match section {
        "recommended" => {
            let cap = if essential { 2 } else { 3 };
            music_recommend::build_recommended_row(
                cookies, &seed, last_search, history, prefer_br, rotation, cap, &mut seen,
            )
        }
        "peers" => {
            let cap = if essential { 3 } else { 5 };
            music_recommend::build_peers_row(
                cookies, &seed, last_search, history, rotation, cap, &mut seen,
            )
        }
        "new_artists" => {
            let take = if essential { 1 } else { 2 };
            music_recommend::build_new_artists_row(
                cookies, &seed, history, last_search, prefer_br, rotation, take, &mut seen,
            )
        }
        "history_mix" => music_recommend::build_made_for_you_row(
            cookies,
            &seed,
            history,
            last_search,
            prefer_br,
            rotation,
            essential,
            &mut seen,
        ),
        other => Err(format!("Secao desconhecida: {other}")),
    }
}

fn feed_context(state: &SharedState) -> (WatchHistory, Option<Video>, String, usize) {
    let history = state.watch_history.lock().clone();
    let raw_search = state.last_search();
    let mut last_search = if raw_search.trim().is_empty() {
        history.feed_context()
    } else {
        raw_search
    };
    last_search = history.effective_search_context(&last_search);

    let seed = history
        .representative_seed()
        .or_else(|| state.last_video.lock().clone())
        .or_else(|| state.queue.lock().current_video())
        .or_else(|| history.music_seed());

    let rotation = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as usize)
        .unwrap_or(0);
    (history, seed, last_search, rotation)
}

#[tauri::command]
pub fn home_feed_local(state: tauri::State<'_, SharedState>) -> FeedLocal {
    let (history, seed, last_search, _) = feed_context(&state);
    build_local_parts(&history, &last_search, seed)
}

#[tauri::command]
pub async fn home_feed_section(
    state: tauri::State<'_, SharedState>,
    section: String,
    exclude_ids: Vec<String>,
    essential: Option<bool>,
) -> Result<FeedSectionResult, String> {
    let essential = essential.unwrap_or(false);
    let (history, seed, last_search, rotation) = feed_context(&state);
    let cookies = state.cookies();
    let section_key = section.trim().to_lowercase();

    tauri::async_runtime::spawn_blocking(move || {
        let items = build_section(
            &cookies,
            &history,
            seed,
            &last_search,
            rotation,
            &section_key,
            &exclude_ids,
            essential,
        )?;
        Ok(FeedSectionResult {
            section: section_key,
            items,
        })
    })
    .await
    .map_err(|e| format!("feed_section: {e}"))?
}

#[tauri::command]
pub async fn home_feed_genres(
    state: tauri::State<'_, SharedState>,
    exclude_ids: Vec<String>,
) -> Result<GenreRowsResult, String> {
    let (history, _, last_search, rotation) = feed_context(&state);
    let cookies = state.cookies();

    tauri::async_runtime::spawn_blocking(move || {
        let rows = build_genre_rows(&cookies, &history, &last_search, rotation, &exclude_ids)?;
        Ok(GenreRowsResult { rows })
    })
    .await
    .map_err(|e| format!("feed_genres: {e}"))?
}
