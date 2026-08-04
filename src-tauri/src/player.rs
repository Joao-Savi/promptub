use crate::queue_refill;
use crate::state::SharedState;
use crate::youtube::{self, Video};

pub(crate) fn track_play(state: &SharedState, video: &Video) {
    state.set_last_video(video.clone());
    state.watch_history.lock().record(video.clone());
    crate::stream::prewarm_queue_ahead(state);
    queue_refill::maybe_refill_queue(state);
}

#[tauri::command]
pub async fn resolve_stream(
    state: tauri::State<'_, SharedState>,
    video_id: String,
    video_url: Option<String>,
) -> Result<String, String> {
    let id = video_id.trim().to_string();
    if youtube::parse_youtube_id(&id).is_none() {
        return Err("ID invalido".into());
    }
    if let Some(cached) = state.stream_cache.get(&id) {
        return Ok(cached);
    }
    let cookies = state.cookies();
    let id_fetch = id.clone();
    let cookies_fetch = cookies.clone();
    let url = tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        let track = track_for_stream(&cookies_fetch, &id_fetch, video_url.as_deref())?;
        crate::stream::resolve_stream_url(&cookies_fetch, &track)
    })
    .await
    .map_err(|e| format!("resolve_stream: {e}"))??;
    state.stream_cache.put(id, url.clone());
    Ok(url)
}

fn track_for_stream(cookies: &str, id: &str, track_url: Option<&str>) -> Result<Video, String> {
    if let Some(url) = track_url.map(str::trim).filter(|u| !u.is_empty()) {
        if !youtube::is_youtube_watch_url(url) {
            return Err("URL de video invalida".into());
        }
        return Ok(Video {
            id: id.to_string(),
            title: String::new(),
            uploader: String::new(),
            duration: String::new(),
            url: url.to_string(),
            thumbnail: String::new(),
            is_live: false,
        });
    }
    crate::youtube::fetch_track(cookies, id)?.ok_or_else(|| "Faixa nao encontrada".into())
}

#[tauri::command]
pub fn play(
    state: tauri::State<'_, SharedState>,
    video: Video,
    set_queue: bool,
    _audio_only: bool,
) -> Result<(), String> {
    {
        let mut queue = state.queue.lock();
        if set_queue {
            queue.play_now(video.clone());
        } else if queue.current_video().is_none() {
            if queue.is_empty() {
                queue.add(video.clone());
            } else {
                let idx = queue.len();
                queue.add(video.clone());
                queue.jump_to(idx);
            }
        }
    }
    track_play(&state, &video);
    Ok(())
}

#[tauri::command]
pub fn next(state: tauri::State<'_, SharedState>) -> Result<Option<Video>, String> {
    let video = {
        let mut queue = state.queue.lock();
        queue.next()
    };
    if let Some(video) = video {
        track_play(&state, &video);
        return Ok(Some(video));
    }
    Ok(None)
}

#[tauri::command]
pub fn prev(state: tauri::State<'_, SharedState>) -> Result<Option<Video>, String> {
    let video = {
        let mut queue = state.queue.lock();
        queue.prev()
    };
    if let Some(video) = video {
        track_play(&state, &video);
        return Ok(Some(video));
    }
    Ok(None)
}

#[tauri::command]
pub fn prewarm_playlist(
    state: tauri::State<'_, SharedState>,
    items: Vec<Video>,
    _audio_only: bool,
) -> Result<(), String> {
    if !items.is_empty() {
        state
            .stream_cache
            .prewarm_async(state.cookies(), items);
    }
    crate::stream::prewarm_queue_ahead(&state);
    Ok(())
}

#[tauri::command]
pub fn prewarm_status(state: tauri::State<'_, SharedState>) -> crate::stream::PrewarmStatus {
    let (done, total) = state.stream_cache.prewarm_status();
    crate::stream::PrewarmStatus { done, total }
}
