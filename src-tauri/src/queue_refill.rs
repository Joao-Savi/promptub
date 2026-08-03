use crate::discover::{
    artist_key, build_music_context_rich, contextual_search_queries,
    filter_relevant, interleave_sources, pick_diverse_candidates, title_fingerprint,
};
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
    let last_search = state.last_search();
    let state = SharedState::clone(state);

    thread::spawn(move || {
        let result = refill_worker(&state, &cookies, &last_search, &seed);
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
) -> Result<usize, String> {
    let rotation = state
        .refill_generation
        .fetch_add(1, Ordering::Relaxed);

    let (exclude, exclude_fps, uploader_counts, alt_seeds, ctx, taste) = {
        let q = state.queue.lock();
        let history = state.watch_history.lock();
        let mut exclude = q.existing_ids();
        for id in history.played_ids() {
            exclude.insert(id);
        }
        for id in history.blocked_ids() {
            exclude.insert(id);
        }
        let mut exclude_fps = history.played_fingerprints();
        for v in &q.snapshot().items {
            exclude_fps.insert(title_fingerprint(v));
        }
        let rich = history.listening_context(last_search, seed);
        let ctx = build_music_context_rich(&rich, seed);
        let taste = history.taste.clone();
        (
            exclude,
            exclude_fps,
            uploader_counts(&q.snapshot().items),
            alternate_seeds(&q, seed),
            ctx,
            taste,
        )
    };

    let queries = contextual_search_queries(last_search, seed, rotation);
    let seed_artist = artist_key(seed);

    let mut sources: Vec<Vec<Video>> = Vec::new();

    match rotation % 3 {
        0 => {
            sources.extend(fetch_searches(cookies, &queries, 4, 8));
            for alt in alt_seeds.iter().take(2) {
                sources.push(fetch_genre_mix(cookies, alt, &ctx, 5)?);
            }
        }
        1 => {
            sources.extend(fetch_searches(cookies, &queries, 3, 10));
            for q in queries.iter().skip(3).take(2) {
                sources.push(youtube::fetch_search_recent(cookies, q, 6)?);
            }
        }
        _ => {
            sources.extend(fetch_searches(cookies, &queries, 5, 6));
            for alt in alt_seeds.iter().take(3) {
                sources.push(fetch_genre_mix(cookies, alt, &ctx, 4)?);
            }
        }
    }

    let interleaved = interleave_sources(sources);
    let filtered = filter_relevant(&ctx, interleaved);
    let picked = pick_diverse_candidates(
        filtered,
        &exclude,
        &exclude_fps,
        &uploader_counts,
        &seed_artist,
        REFILL_BATCH,
        Some(&ctx),
        Some(&taste),
    );

    if picked.is_empty() {
        return Ok(0);
    }

    let added = state.queue.lock().append_unique(picked);
    Ok(added)
}

fn fetch_genre_mix(
    cookies: &str,
    video: &Video,
    ctx: &crate::discover::MusicContext,
    limit: usize,
) -> Result<Vec<Video>, String> {
    let items = youtube::fetch_mix(cookies, &video.id, limit)?;
    Ok(filter_relevant(ctx, items))
}

fn fetch_searches(
    cookies: &str,
    queries: &[String],
    take_queries: usize,
    per_query: usize,
) -> Vec<Vec<Video>> {
    queries
        .iter()
        .take(take_queries)
        .filter_map(|q| youtube::fetch_search(cookies, q, per_query).ok())
        .collect()
}

fn alternate_seeds(queue: &Queue, current: &Video) -> Vec<Video> {
    let cur_artist = artist_key(current);
    let mut different = Vec::new();
    let mut any = Vec::new();

    for v in queue.upcoming_from_current(10) {
        any.push(v.clone());
        if artist_key(&v) != cur_artist {
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
        let u = artist_key(v);
        if u.is_empty() {
            continue;
        }
        *map.entry(u).or_insert(0) += 1;
    }
    map
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
