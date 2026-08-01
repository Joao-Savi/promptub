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
pub async fn recommended_playlist(
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
    let query_owned = seed_query.clone().or_else(|| {
        if fallback_query.is_empty() {
            None
        } else {
            Some(fallback_query)
        }
    });

    let (items, seed_label) = tauri::async_runtime::spawn_blocking(move || {
        build_playlist(
            &cookies,
            seed_id.as_deref(),
            query_owned.as_deref(),
        )
    })
    .await
    .map_err(|e| format!("playlist: {e}"))??;

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
        let title = playlist[0].title.clone();

        let genre_queries = crate::discover::genre_search_queries(
            seed_query.unwrap_or(""),
            &playlist[0],
            0,
        );
        let primary_genre = genre_queries.first().cloned();

        let cookies_owned = cookies.to_string();
        let (mix_res, genre_res, title_res) = std::thread::scope(|s| {
            let c1 = cookies_owned.clone();
            let c2 = cookies_owned.clone();
            let c3 = cookies_owned.clone();
            let sid = seed_id.clone();
            let gq = primary_genre.clone();
            let tq = {
                let t = simplify_for_search(&title);
                if t.len() >= 4 { Some(t) } else { None }
            };

            let h_mix = s.spawn(move || youtube::fetch_mix(&c1, &sid, 22));
            let h_genre = s.spawn(move || match gq {
                Some(q) if q.len() > 3 => youtube::fetch_search(&c2, &q, 10),
                _ => Ok(vec![]),
            });
            let h_title = s.spawn(move || match tq {
                Some(q) => youtube::fetch_search(&c3, &q, 6),
                _ => Ok(vec![]),
            });
            (
                h_mix.join().unwrap(),
                h_genre.join().unwrap(),
                h_title.join().unwrap(),
            )
        });

        push_many(&mut playlist, &mut seen, mix_res?);
        push_many(&mut playlist, &mut seen, genre_res?);
        push_many(&mut playlist, &mut seen, title_res?);

        if playlist.len() < PLAYLIST_TARGET {
            for q in genre_queries.into_iter().skip(1).take(2) {
                push_many(
                    &mut playlist,
                    &mut seen,
                    youtube::fetch_search(cookies, &q, 6)?,
                );
                if playlist.len() >= PLAYLIST_TARGET {
                    break;
                }
            }
        }

        if playlist.len() < PLAYLIST_TARGET {
            if let Some(anchor) = playlist.get(1).or(playlist.first()) {
                let anchor_id = anchor.id.clone();
                push_many(
                    &mut playlist,
                    &mut seen,
                    youtube::fetch_mix(cookies, &anchor_id, 8)?,
                );
            }
        }
    } else if let Some(q) = seed_query {
        let cookies_owned = cookies.to_string();
        let q_owned = q.to_string();
        let (search_res, mix_res) = std::thread::scope(|s| {
            let c1 = cookies_owned.clone();
            let c2 = cookies_owned.clone();
            let c3 = cookies_owned.clone();
            let q1 = q_owned.clone();
            let q2 = q_owned.clone();
            let h_search = s.spawn(move || youtube::fetch_search(&c1, &q1, 12));
            let h_mix = s.spawn(move || {
                let first = youtube::fetch_search(&c2, &q2, 1)?;
                if let Some(v) = first.into_iter().next() {
                    youtube::fetch_mix(&c3, &v.id, 18)
                } else {
                    Ok(vec![])
                }
            });
            (h_search.join().unwrap(), h_mix.join().unwrap())
        });
        push_many(&mut playlist, &mut seen, search_res?);
        push_many(&mut playlist, &mut seen, mix_res?);
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
    crate::discover::simplify_for_search(title)
}
