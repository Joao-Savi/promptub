//! Preferencias explicitas (like/dislike) e limpeza da fila.

use crate::discover::artist_key;
use crate::history::TasteState;
use crate::queue_refill;
use crate::state::SharedState;
use crate::youtube::Video;
use serde::Serialize;
use tauri::{Emitter, State};

#[derive(Clone, Serialize)]
pub struct TasteStatus {
    pub video_id: String,
    pub state: String,
}

fn status_label(state: TasteState) -> String {
    match state {
        TasteState::Liked => "liked".into(),
        TasteState::Disliked => "disliked".into(),
        TasteState::None => "none".into(),
    }
}

pub fn purge_disliked_from_queue(state: &SharedState) {
    let (blocked_tracks, blocked_artists) = {
        let h = state.watch_history.lock();
        (
            h.taste.disliked_tracks.clone(),
            h.taste.disliked_artists.clone(),
        )
    };

    let removed = {
        let mut q = state.queue.lock();
        q.purge_where(&|v| {
            let artist = artist_key(v);
            blocked_tracks.contains(&v.id) || blocked_artists.contains(&artist)
        })
    };

    if removed > 0 {
        if let Some(handle) = state.app_handle.lock().clone() {
            let snapshot = state.queue.lock().snapshot();
            let _ = handle.emit("queue-updated", snapshot);
        }
    }
}

#[tauri::command]
pub fn taste_like(state: State<'_, SharedState>, video: Video) -> Result<TasteStatus, String> {
    state.watch_history.lock().like(&video);
    Ok(TasteStatus {
        video_id: video.id.clone(),
        state: status_label(TasteState::Liked),
    })
}

#[tauri::command]
pub fn taste_dislike(state: State<'_, SharedState>, video: Video) -> Result<TasteStatus, String> {
    state.watch_history.lock().dislike(&video);
    purge_disliked_from_queue(&state);
    queue_refill::maybe_refill_queue(&state);
    Ok(TasteStatus {
        video_id: video.id.clone(),
        state: status_label(TasteState::Disliked),
    })
}

#[tauri::command]
pub fn taste_get(state: State<'_, SharedState>, video: Video) -> Result<TasteStatus, String> {
    let history = state.watch_history.lock();
    Ok(TasteStatus {
        video_id: video.id.clone(),
        state: status_label(history.taste_state(&video)),
    })
}
