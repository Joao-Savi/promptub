use crate::discover::{genre_search_queries, normalize_uploader};
use crate::queue::{Queue, QueueSnapshot};
use crate::state::SharedState;
use crate::stream;
use crate::youtube::{self, Video};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::thread;
use tauri::Emitter;

pub const REFILL_THRESHOLD: usize = 3;
pub const REFILL_BATCH: usize = 12;

pub fn maybe_refill_queue(state: &SharedState) {
    let remaining = state.queue.lock().remaining_after_current();
    if remaining > REFILL_THRESHOLD {
        return;
    }

    let seed = state
        .queue
        .lock()
        .current_video()
        .or_else(|| state.last_video.lock().clone());
    let Some(seed) = seed else {
        return;
    };

    if state
        .refill_in_progress
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    let cookies = state.cookies();
    let audio_only = *state.audio_only.lock();
    let last_search = state.last_search();
    let state = SharedState::clone(state);

    thread::spawn(move || {
        let result = refill_worker(&state, &cookies, &last_search, &seed, audio_only);
        state.refill_in_progress.store(false, Ordering::SeqCst);

        if let Ok(added) = result {
            if added > 0 {
                emit_queue_updated(&state, added);
                stream::prewarm_queue_ahead(&state);
            }
        }

        if state.queue.lock().remaining_after_current() <= REFILL_THRESHOLD {
            maybe_refill_queue(&state);
        }
    });
}

fn refill_worker(
    state: &SharedState,
    cookies: &str,
    last_search: &str,
    seed: &Video,
    audio_only: bool,
) -> Result<usize, String> {
    let rotation = state
        .refill_generation
        .fetch_add(1, Ordering::Relaxed);

    let (exclude, uploader_counts, alt_seeds) = {
        let q = state.queue.lock();
        (
            q.existing_ids(),
            uploader_counts(&q.snapshot().items),
            alternate_seeds(&q, seed),
        )
    };

    let queries = if audio_only {
        genre_search_queries(last_search, seed, rotation)
    } else {
        let history = state.watch_history.lock().clone();
        crate::video_recommend::queue_refill_queries(last_search, seed, rotation, &history)
    };
    let seed_artist = normalize_uploader(&seed.uploader);

    let mut sources: Vec<Vec<Video>> = Vec::new();

    match rotation % 3 {
        0 => {
            sources.extend(fetch_searches(cookies, &queries, 2, 8, audio_only));
            if let Some(alt) = alt_seeds.first() {
                sources.push(fetch_related(cookies, alt, audio_only, 10)?);
            }
        }
        1 => {
            sources.push(fetch_related(cookies, seed, audio_only, 6)?);
            sources.extend(fetch_searches(cookies, &queries, 1, 10, audio_only));
            if let Some(q) = queries.get(2) {
                sources.push(youtube::fetch_search(cookies, q, 8)?);
            }
        }
        _ => {
            sources.extend(fetch_searches(cookies, &queries, 3, 6, audio_only));
            for alt in alt_seeds.iter().take(2) {
                sources.push(fetch_related(cookies, alt, audio_only, 6)?);
            }
        }
    }

    let interleaved = interleave_sources(sources);
    let picked = pick_diverse(
        interleaved,
        &exclude,
        &uploader_counts,
        &seed_artist,
        REFILL_BATCH,
    );

    if picked.is_empty() {
        return Ok(0);
    }

    let added = state.queue.lock().append_unique(picked);
    Ok(added)
}

fn fetch_related(cookies: &str, video: &Video, audio_only: bool, limit: usize) -> Result<Vec<Video>, String> {
    if audio_only {
        youtube::fetch_mix(cookies, &video.id, limit)
    } else {
        youtube::fetch_rd(cookies, &video.id, limit)
    }
}

fn fetch_searches(
    cookies: &str,
    queries: &[String],
    take_queries: usize,
    per_query: usize,
    audio_only: bool,
) -> Vec<Vec<Video>> {
    queries
        .iter()
        .take(take_queries)
        .filter_map(|q| {
            if audio_only {
                youtube::fetch_search(cookies, q, per_query).ok()
            } else {
                youtube::fetch_search_recent(cookies, q, per_query)
                    .or_else(|_| youtube::fetch_search(cookies, q, per_query))
                    .ok()
            }
        })
        .collect()
}

fn alternate_seeds(queue: &Queue, current: &Video) -> Vec<Video> {
    let cur_artist = normalize_uploader(&current.uploader);
    let mut different = Vec::new();
    let mut any = Vec::new();

    for v in queue.upcoming_from_current(10) {
        any.push(v.clone());
        if normalize_uploader(&v.uploader) != cur_artist {
            different.push(v);
        }
    }

    if !different.is_empty() {
        different.truncate(3);
        return different;
    }
    any.truncate(2);
    any
}

fn uploader_counts(items: &[Video]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for v in items {
        let u = normalize_uploader(&v.uploader);
        if u.is_empty() {
            continue;
        }
        *map.entry(u).or_insert(0) += 1;
    }
    map
}

fn interleave_sources(sources: Vec<Vec<Video>>) -> Vec<Video> {
    let mut out = Vec::new();
    let mut idx = vec![0usize; sources.len()];
    loop {
        let mut progressed = false;
        for (i, src) in sources.iter().enumerate() {
            if idx[i] < src.len() {
                out.push(src[idx[i]].clone());
                idx[i] += 1;
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    out
}

fn pick_diverse(
    candidates: Vec<Video>,
    exclude: &HashSet<String>,
    queue_counts: &HashMap<String, usize>,
    seed_artist: &str,
    limit: usize,
) -> Vec<Video> {
    let queue_len = queue_counts.values().sum::<usize>().max(1);
    let mut batch_counts: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::new();

    for v in candidates {
        if exclude.contains(&v.id) {
            continue;
        }
        let artist = normalize_uploader(&v.uploader);
        if artist.is_empty() {
            out.push(v);
            if out.len() >= limit {
                break;
            }
            continue;
        }

        let in_queue = queue_counts.get(&artist).copied().unwrap_or(0);
        let in_batch = batch_counts.get(&artist).copied().unwrap_or(0);

        let max_batch = if artist == seed_artist { 1 } else { 2 };
        let queue_share = in_queue as f32 / queue_len as f32;
        if queue_share > 0.45 && in_batch >= 1 {
            continue;
        }
        if in_batch >= max_batch {
            continue;
        }

        batch_counts.insert(artist, in_batch + 1);
        out.push(v);
        if out.len() >= limit {
            break;
        }
    }

    out
}

fn emit_queue_updated(state: &SharedState, added: usize) {
    let handle = state.app_handle.lock().clone();
    let Some(handle) = handle else {
        return;
    };
    let snapshot: QueueSnapshot = state.queue.lock().snapshot();
    let _ = handle.emit("queue-updated", snapshot);
    let _ = handle.emit("queue-refill", added);
}
