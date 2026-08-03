//! Recomendacoes de musica com variedade por genero/artista (estilo YouTube Music).

use crate::discover::{
    artist_key, build_music_context, contextual_search_queries, cold_start_queries,
    extract_artist_label, filter_playable, filter_relevant, interleave_sources,
    peer_artists, pick_diverse_candidates, pick_new_artists,
    refine_search_results,
};
use crate::history::WatchHistory;
use crate::youtube::{self, Video};
use std::collections::{HashMap, HashSet};
use std::thread;

const RECOMMENDED_LIMIT: usize = 12;
const PEERS_LIMIT: usize = 10;
const NEW_ARTISTS_LIMIT: usize = 10;
const MADE_FOR_YOU_LIMIT: usize = 10;

pub fn seed_label(history: &WatchHistory, last_search: &str, seed: &Option<Video>) -> String {
    if !last_search.trim().is_empty() {
        if history.recent_music.is_empty() {
            format!("explorar · {}", last_search.trim())
        } else {
            format!("seu gosto · {}", last_search.trim())
        }
    } else if let Some(v) = seed.as_ref() {
        format!("continuar · {}", extract_artist_label(v))
    } else if !history.recent_music.is_empty() {
        "baseado no seu historico".into()
    } else if history.prefers_brazilian() {
        "explorar · musicas brasileiras".into()
    } else {
        "explorar · musicas".into()
    }
}

fn parallel_fetch_searches(cookies: &str, queries: &[String], per_query: usize) -> Vec<Vec<Video>> {
    if queries.is_empty() {
        return vec![];
    }
    let cookies = cookies.to_string();
    let handles: Vec<_> = queries
        .iter()
        .map(|q| {
            let q = q.clone();
            let cookies = cookies.clone();
            thread::spawn(move || youtube::fetch_search(&cookies, &q, per_query).unwrap_or_default())
        })
        .collect();
    handles.into_iter().filter_map(|h| h.join().ok()).collect()
}

fn parallel_fetch_searches_recent(cookies: &str, queries: &[String], per_query: usize) -> Vec<Vec<Video>> {
    if queries.is_empty() {
        return vec![];
    }
    let cookies = cookies.to_string();
    let handles: Vec<_> = queries
        .iter()
        .map(|q| {
            let q = q.clone();
            let cookies = cookies.clone();
            thread::spawn(move || {
                youtube::fetch_search_recent(&cookies, &q, per_query).unwrap_or_default()
            })
        })
        .collect();
    handles.into_iter().filter_map(|h| h.join().ok()).collect()
}

pub fn build_recommended_row(
    cookies: &str,
    seed: &Option<Video>,
    last_search: &str,
    prefer_br: bool,
    rotation: usize,
    query_cap: usize,
    seen: &mut HashSet<String>,
) -> Result<Vec<Video>, String> {
    if let Some(s) = seed {
        let ctx = build_music_context(last_search, s);
        let queries: Vec<String> = contextual_search_queries(last_search, s, rotation)
            .into_iter()
            .take(query_cap)
            .collect();
        let mut sources = parallel_fetch_searches(cookies, &queries, 6);
        sources = sources
            .into_iter()
            .map(|items| filter_relevant(&ctx, items))
            .collect();
        if query_cap > 3 {
            if let Ok(mix) = youtube::fetch_mix(cookies, &s.id, 8) {
                sources.push(filter_relevant(&ctx, mix));
            }
        }

        let interleaved = interleave_sources(sources);
        let filtered = filter_relevant(&ctx, interleaved);
        let seed_artist = artist_key(s);
        let mut picked = pick_diverse_candidates(
            filtered, seen, &HashMap::new(), &seed_artist, RECOMMENDED_LIMIT,
        );
        if picked.is_empty() {
            picked = cold_start_fallback(cookies, prefer_br, rotation, seen, query_cap.min(3))?;
        }
        for v in &picked {
            seen.insert(v.id.clone());
        }
        return Ok(picked);
    }

    if !last_search.trim().is_empty() {
        let ls = last_search.trim();
        let raw = youtube::fetch_search(cookies, ls, 10)?;
        let refined = refine_search_results(ls, raw);
        if let Some(s) = refined.first() {
            let ctx = build_music_context(ls, s);
            let queries: Vec<String> = contextual_search_queries(ls, s, rotation)
                .into_iter()
                .take(query_cap.min(5))
                .collect();
            let sources: Vec<Vec<Video>> = parallel_fetch_searches(cookies, &queries, 6)
                .into_iter()
                .map(|items| filter_relevant(&ctx, items))
                .collect();
            let interleaved = interleave_sources(sources);
            let filtered = filter_relevant(&ctx, interleaved);
            let mut picked = pick_diverse_candidates(
                filtered,
                seen,
                &HashMap::new(),
                &artist_key(s),
                RECOMMENDED_LIMIT,
            );
            if picked.is_empty() {
                picked = cold_start_fallback(cookies, prefer_br, rotation, seen, query_cap.min(3))?;
            }
            for v in &picked {
                seen.insert(v.id.clone());
            }
            return Ok(picked);
        }
    }

    cold_start_fallback(cookies, prefer_br, rotation, seen, query_cap)
}

/// Artistas do mesmo genero — 1 faixa por artista parecido.
pub fn build_peers_row(
    cookies: &str,
    seed: &Option<Video>,
    last_search: &str,
    rotation: usize,
    peer_cap: usize,
    seen: &mut HashSet<String>,
) -> Result<Vec<Video>, String> {
    let Some(s) = seed else {
        return Ok(vec![]);
    };

    let ctx = build_music_context(last_search, s);
    let peers: Vec<String> = peer_artists(s, rotation, peer_cap);
    let seed_artist = artist_key(s);

    let peer_queries: Vec<String> = peers
        .iter()
        .map(|peer| format!("{peer} musica"))
        .collect();
    let search_results = parallel_fetch_searches(cookies, &peer_queries, 4);

    let mut out = Vec::new();
    for items in search_results {
        if out.len() >= PEERS_LIMIT {
            break;
        }
        let filtered = filter_relevant(&ctx, items);
        let picked = pick_diverse_candidates(
            filtered,
            seen,
            &HashMap::new(),
            &seed_artist,
            1,
        );
        if let Some(v) = picked.into_iter().next() {
            seen.insert(v.id.clone());
            out.push(v);
        }
    }
    Ok(out)
}

fn cold_start_fallback(
    cookies: &str,
    prefer_br: bool,
    rotation: usize,
    seen: &HashSet<String>,
    max_queries: usize,
) -> Result<Vec<Video>, String> {
    let mut sources: Vec<Vec<Video>> = Vec::new();
    let queries: Vec<String> = cold_start_queries(prefer_br, rotation)
        .into_iter()
        .take(max_queries)
        .collect();
    sources.extend(
        parallel_fetch_searches(cookies, &queries, 8)
            .into_iter()
            .map(filter_playable),
    );
    if sources.is_empty() {
        let fallback = cold_start_queries(false, rotation + 1);
        sources.extend(
            parallel_fetch_searches(cookies, &fallback, 8)
                .into_iter()
                .map(filter_playable),
        );
    }
    let interleaved = interleave_sources(sources);
    let picked = pick_diverse_candidates(
        interleaved,
        seen,
        &HashMap::new(),
        "",
        RECOMMENDED_LIMIT,
    );
    Ok(picked)
}

pub fn build_new_artists_row(
    cookies: &str,
    seed: &Option<Video>,
    history: &WatchHistory,
    last_search: &str,
    prefer_br: bool,
    rotation: usize,
    query_take: usize,
    seen: &mut HashSet<String>,
) -> Result<Vec<Video>, String> {
    let known = history.known_uploaders();
    let mut sources: Vec<Vec<Video>> = Vec::new();

    if let Some(s) = seed {
        let ctx = build_music_context(last_search, s);
        let queries: Vec<String> = contextual_search_queries(last_search, s, rotation + 3)
            .into_iter()
            .take(query_take)
            .collect();
        let sources: Vec<Vec<Video>> = parallel_fetch_searches_recent(cookies, &queries, 6)
            .into_iter()
            .map(|items| filter_relevant(&ctx, items))
            .collect();
        let interleaved = interleave_sources(sources);
        let filtered = filter_relevant(&ctx, interleaved);
        let picked = pick_new_artists(filtered, seen, &known, NEW_ARTISTS_LIMIT);
        for v in &picked {
            seen.insert(v.id.clone());
        }
        return Ok(picked);
    }

    let queries: Vec<String> = cold_start_queries(prefer_br, rotation + 2)
        .into_iter()
        .take(query_take)
        .collect();
    sources.extend(
        parallel_fetch_searches_recent(cookies, &queries, 8)
            .into_iter()
            .map(filter_playable),
    );

    let interleaved = interleave_sources(sources);
    let picked = pick_new_artists(interleaved, seen, &known, NEW_ARTISTS_LIMIT);
    for v in &picked {
        seen.insert(v.id.clone());
    }
    Ok(picked)
}

/// Feito pra voce — com historico usa o gosto; sem historico monta descobertas.
pub fn build_made_for_you_row(
    cookies: &str,
    seed: &Option<Video>,
    history: &WatchHistory,
    last_search: &str,
    prefer_br: bool,
    rotation: usize,
    quick: bool,
    seen: &mut HashSet<String>,
) -> Result<Vec<Video>, String> {
    if history.recent_music.is_empty() {
        return build_made_for_you_cold(cookies, seed, prefer_br, rotation, quick, seen);
    }

    let kw_take = if quick { 1 } else { 3 };
    let keywords = history.interest_keywords(6);
    let top = history.top_music(3);
    if top.is_empty() {
        return build_made_for_you_cold(cookies, seed, prefer_br, rotation, quick, seen);
    }

    let mut sources: Vec<Vec<Video>> = Vec::new();
    let ctx = build_music_context(last_search, &top[0]);

    let kw_queries: Vec<String> = keywords
        .iter()
        .take(kw_take)
        .map(|kw| format!("{kw} musica"))
        .collect();
    sources.extend(
        parallel_fetch_searches(cookies, &kw_queries, 6)
            .into_iter()
            .map(|items| filter_relevant(&ctx, items)),
    );

    let ctx_queries: Vec<String> = contextual_search_queries(last_search, &top[0], rotation + 1)
        .into_iter()
        .take(if quick { 1 } else { 3 })
        .collect();
    sources.extend(
        parallel_fetch_searches(cookies, &ctx_queries, 6)
            .into_iter()
            .map(|items| filter_relevant(&ctx, items)),
    );
    if !quick {
        for anchor in top.iter().take(2) {
            if let Ok(mix) = youtube::fetch_mix(cookies, &anchor.id, 5) {
                sources.push(filter_relevant(&ctx, mix));
            }
        }
    }

    let interleaved = interleave_sources(sources);
    let filtered = filter_relevant(&ctx, interleaved);
    let picked = pick_diverse_candidates(
        filtered,
        seen,
        &HashMap::new(),
        &artist_key(&top[0]),
        MADE_FOR_YOU_LIMIT,
    );
    for v in &picked {
        seen.insert(v.id.clone());
    }
    Ok(picked)
}

fn build_made_for_you_cold(
    cookies: &str,
    seed: &Option<Video>,
    prefer_br: bool,
    rotation: usize,
    quick: bool,
    seen: &mut HashSet<String>,
) -> Result<Vec<Video>, String> {
    let mut sources: Vec<Vec<Video>> = Vec::new();

    if let Some(s) = seed {
        let ctx = build_music_context("", s);
        for peer in peer_artists(s, rotation, if quick { 2 } else { 5 }) {
            if let Ok(items) = youtube::fetch_search(cookies, &format!("{peer} musica"), 4) {
                sources.push(filter_relevant(&ctx, items));
            }
        }
    }

    for q in cold_start_queries(prefer_br, rotation + 1)
        .into_iter()
        .take(if quick { 2 } else { 5 })
    {
        if let Ok(items) = youtube::fetch_search(cookies, &q, 6) {
            sources.push(filter_playable(items));
        }
    }

    let interleaved = interleave_sources(sources);
    let seed_artist = seed.as_ref().map(artist_key).unwrap_or_default();
    let picked = pick_diverse_candidates(
        interleaved,
        seen,
        &HashMap::new(),
        &seed_artist,
        MADE_FOR_YOU_LIMIT,
    );
    for v in &picked {
        seen.insert(v.id.clone());
    }
    Ok(picked)
}

/// Playlist completa baseada no historico + genero (para [ REC.PL ]).
pub fn build_history_playlist(
    cookies: &str,
    history: &WatchHistory,
    seed: Option<Video>,
    seed_query: Option<&str>,
    last_search: &str,
) -> Result<(Vec<Video>, String), String> {
    let seen = history.played_ids();
    let anchor = seed
        .or_else(|| history.music_seed())
        .or_else(|| {
            seed_query
                .filter(|q| !q.is_empty())
                .and_then(|q| youtube::fetch_search(cookies, q, 1).ok())
                .and_then(|v| v.into_iter().next())
        });

    let seed_label = anchor
        .as_ref()
        .map(|v| extract_artist_label(v))
        .or_else(|| seed_query.map(|s| s.to_string()))
        .unwrap_or_else(|| "Feito pra voce".into());

    let Some(anchor) = anchor else {
        return Err("Toque algo ou faca uma busca para gerar a playlist.".into());
    };

    let rotation = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as usize)
        .unwrap_or(0);

    let mut sources: Vec<Vec<Video>> = Vec::new();
    let ctx = build_music_context(last_search, &anchor);

    for q in contextual_search_queries(last_search, &anchor, rotation).iter().take(6) {
        if let Ok(items) = youtube::fetch_search(cookies, q, 6) {
            sources.push(filter_relevant(&ctx, items));
        }
    }
    for kw in history.interest_keywords(4) {
        if let Ok(items) = youtube::fetch_search(cookies, &format!("{kw} musica"), 6) {
            sources.push(filter_relevant(&ctx, items));
        }
    }
    if let Ok(mix) = youtube::fetch_mix(cookies, &anchor.id, 8) {
        sources.push(filter_relevant(&ctx, mix));
    }

    let interleaved = interleave_sources(sources);
    let filtered = filter_relevant(&ctx, interleaved);
    let mut playlist = pick_diverse_candidates(
        filtered,
        &seen,
        &HashMap::new(),
        &artist_key(&anchor),
        25,
    );

    if playlist.is_empty() {
        return Err("Nao foi possivel montar a playlist.".into());
    }

    if !playlist.iter().any(|v| v.id == anchor.id) {
        playlist.insert(0, anchor.clone());
    }

    playlist.truncate(25);
    Ok((playlist, seed_label))
}
