use crate::music_recommend;
use crate::state::SharedState;
use crate::youtube::{self, Video};
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub struct RecommendedPlaylist {
    pub items: Vec<Video>,
    pub seed_label: String,
    pub count: usize,
}

#[tauri::command]
pub async fn recommended_playlist(
    state: State<'_, SharedState>,
    seed_video_id: Option<String>,
    seed_query: Option<String>,
    mode: Option<String>,
) -> Result<RecommendedPlaylist, String> {
    let cookies = state.cookies();
    let history = state.watch_history.lock().clone();
    let last_search = state.last_search();
    let playlist_mode = mode.unwrap_or_else(|| "personal".into());

    let seed_id = seed_video_id
        .filter(|s| !s.is_empty())
        .or_else(|| state.last_video_id())
        .or_else(|| state.queue.lock().current_video().map(|v| v.id));

    let fallback_query = seed_query.or_else(|| {
        let q = state.last_search();
        if q.is_empty() {
            None
        } else {
            Some(q)
        }
    });

    let (items, seed_label) = tauri::async_runtime::spawn_blocking(move || {
        match playlist_mode.as_str() {
            "artist" => {
                let q = fallback_query.ok_or_else(|| {
                    "Digite o nome do artista na busca para montar a playlist.".to_string()
                })?;
                music_recommend::build_artist_playlist(&cookies, &q, &history)
            }
            "mixed" => music_recommend::build_mixed_playlist(&cookies, &history, &last_search),
            "discoveries" => music_recommend::build_discoveries_playlist(&cookies, &history, &last_search),
            _ => {
                let seed_video = if let Some(ref id) = seed_id {
                    youtube::fetch_track(&cookies, id)?.or(None)
                } else {
                    None
                };
                music_recommend::build_history_playlist(
                    &cookies,
                    &history,
                    seed_video,
                    fallback_query.as_deref(),
                    &last_search,
                )
            }
        }
    })
    .await
    .map_err(|e| format!("playlist: {e}"))??;

    if items.is_empty() {
        return Err("Nao foi possivel montar a playlist.".into());
    }

    let count = items.len();
    Ok(RecommendedPlaylist {
        items,
        seed_label,
        count,
    })
}
