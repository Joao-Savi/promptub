//! Motor de recomendacao estilo YouTube: multiplas fontes, ranking, diversidade.
//! Vies de esquerda e suave (boost), nunca filtro unico.

use crate::discover::{normalize_uploader, simplify_for_search};
use crate::history::WatchHistory;
use crate::youtube::{self, Video, HOME_CHANNEL_LIMIT, HOME_FEED_LIMIT, HOME_LIVE_LIMIT, HOME_REC_LIMIT};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SourceKind {
    Subscription,
    YtHistory,
    Radio,
    ChannelFresh,
    InterestFresh,
    RelatedSearch,
    Explore,
}

struct Candidate {
    video: Video,
    source: SourceKind,
    /// Posicao na lista da fonte (0 = mais relevante / mais recente).
    rank_in_source: usize,
}

const LEFT_SIGNALS: &[&str] = &[
    "marx",
    "marxismo",
    "marxista",
    "comunismo",
    "comunista",
    "socialismo",
    "socialista",
    "lenin",
    "leninismo",
    "proletari",
    "trabalhador",
    "trabalhadora",
    "sindicato",
    "sindical",
    "capitalismo",
    "anticapital",
    "imperialismo",
    "esquerda",
    "revolucao",
    "revolucion",
    "materialismo",
    "classe",
    "operari",
    "operaria",
    "greve",
    "solidariedade",
    "cooperativ",
    "anti-fasc",
    "antifasc",
    "staling",
    "lenin",
    "engels",
    "fidel",
    "che guevara",
    "partido comunista",
    "movimento operario",
    "economia politica",
];

/// Pool pequeno de exploracao com vies de esquerda (~15% do feed).
const EXPLORE_QUERIES: &[&str] = &[
    "marxismo explicado",
    "critica ao capitalismo",
    "historia movimento operario",
    "cinema sovietico",
    "cultura popular esquerda",
    "economia politica marxista",
    "documentario trabalhadores",
    "esquerda brasil",
    "analise politica atual",
    "sindicalismo brasil",
    "materialismo historico",
    "humor critica capitalismo",
    "ciencia marxismo",
    "literatura proletaria",
    "cooperativismo",
];

fn source_weight(kind: SourceKind) -> f32 {
    match kind {
        SourceKind::Subscription => 1.0,
        SourceKind::YtHistory => 0.92,
        SourceKind::Radio => 0.88,
        SourceKind::ChannelFresh => 0.95,
        SourceKind::InterestFresh => 0.82,
        SourceKind::RelatedSearch => 0.78,
        SourceKind::Explore => 0.55,
    }
}

fn left_bias_score(title: &str) -> f32 {
    let t = title.to_lowercase();
    let hits = LEFT_SIGNALS.iter().filter(|s| t.contains(*s)).count();
    if hits == 0 {
        0.0
    } else {
        (hits as f32 * 0.06).min(0.18)
    }
}

fn topic_overlap(title: &str, keywords: &[String]) -> f32 {
    if keywords.is_empty() {
        return 0.0;
    }
    let simplified = simplify_for_search(title);
    let words: HashSet<_> = simplified
        .split_whitespace()
        .filter(|w| w.len() > 3)
        .collect();
    if words.is_empty() {
        return 0.0;
    }
    let hits = keywords.iter().filter(|k| words.contains(k.as_str())).count();
    (hits as f32 / keywords.len() as f32).min(1.0) * 0.25
}

fn freshness_bonus(rank_in_source: usize, kind: SourceKind) -> f32 {
    let base = match kind {
        SourceKind::ChannelFresh | SourceKind::InterestFresh => 0.14,
        SourceKind::Subscription => 0.08,
        _ => 0.04,
    };
    base * (1.0 / (1.0 + rank_in_source as f32 * 0.35))
}

fn score_candidate(c: &Candidate, history: &WatchHistory, keywords: &[String]) -> f32 {
    let mut score = source_weight(c.source);
    score += freshness_bonus(c.rank_in_source, c.source);
    score += history.uploader_score(&c.video.uploader, false) * 0.28;
    score += topic_overlap(&c.video.title, keywords);
    score += left_bias_score(&c.video.title);

    let seen = history
        .recent_video
        .iter()
        .any(|e| e.video.id == c.video.id);
    if !seen {
        score += 0.06;
    }

    score
}

fn push_candidates(
    out: &mut Vec<Candidate>,
    items: Vec<Video>,
    source: SourceKind,
) {
    for (i, video) in items.into_iter().enumerate() {
        out.push(Candidate {
            video,
            source,
            rank_in_source: i,
        });
    }
}

fn pick_ranked(
    mut scored: Vec<(Candidate, f32)>,
    limit: usize,
    max_per_uploader: usize,
) -> Vec<Video> {
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut out = Vec::with_capacity(limit);
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut source_counts: HashMap<SourceKind, usize> = HashMap::new();
    let explore_cap = (limit / 6).max(2);

    for (c, _) in scored {
        if out.len() >= limit {
            break;
        }
        if c.source == SourceKind::Explore {
            let n = source_counts.get(&SourceKind::Explore).copied().unwrap_or(0);
            if n >= explore_cap {
                continue;
            }
        }

        let uploader = normalize_uploader(&c.video.uploader);
        if !uploader.is_empty() {
            let n = counts.get(&uploader).copied().unwrap_or(0);
            if n >= max_per_uploader {
                continue;
            }
            counts.insert(uploader, n + 1);
        }

        *source_counts.entry(c.source).or_insert(0) += 1;
        out.push(c.video);
    }

    out
}

fn collect_candidates(
    cookies: &str,
    seed: Option<&Video>,
    history: &WatchHistory,
    rotation: usize,
    for_sidebar: bool,
) -> Vec<Candidate> {
    let has_cookies = !cookies.is_empty();
    let keywords = history.interest_keywords(false, 6);
    let mut candidates = Vec::new();

    let cookies_owned = cookies.to_string();
    let seed_id = seed.map(|v| v.id.clone());
    let recent_ids: Vec<String> = history
        .recent_videos(4)
        .into_iter()
        .map(|v| v.id)
        .collect();
    let uploaders: Vec<String> = history.top_uploaders(false, 5);
    let interest = keywords.clone();

    std::thread::scope(|s| {
        let mut handles = Vec::new();

        if has_cookies {
            let c = cookies_owned.clone();
            handles.push(s.spawn(move || {
                ("sub".to_string(), youtube::fetch_subscriptions(&c, HOME_FEED_LIMIT + 4))
            }));
            let c = cookies_owned.clone();
            handles.push(s.spawn(move || {
                ("hist".to_string(), youtube::fetch_yt_history(&c, HOME_FEED_LIMIT))
            }));
        }

        if let Some(id) = seed_id.clone() {
            let c = cookies_owned.clone();
            handles.push(s.spawn(move || {
                ("radio_seed".to_string(), youtube::fetch_rd(&c, &id, HOME_REC_LIMIT))
            }));
        }

        for (i, rid) in recent_ids.iter().take(3).enumerate() {
            if seed_id.as_deref() == Some(rid.as_str()) {
                continue;
            }
            let c = cookies_owned.clone();
            let id = rid.clone();
            handles.push(s.spawn(move || {
                (format!("radio_{i}"), youtube::fetch_rd(&c, &id, 8))
            }));
        }

        for (i, uploader) in uploaders.iter().take(4).enumerate() {
            let c = cookies_owned.clone();
            let q = uploader.clone();
            handles.push(s.spawn(move || {
                (
                    format!("ch_{i}"),
                    youtube::fetch_search_recent(&c, &q, 5),
                )
            }));
        }

        for (i, kw) in interest.iter().take(3).enumerate() {
            let c = cookies_owned.clone();
            let q = kw.clone();
            handles.push(s.spawn(move || {
                (
                    format!("int_{i}"),
                    youtube::fetch_search_recent(&c, &q, 6),
                )
            }));
        }

        if let Some(v) = seed {
            let theme = simplify_for_search(&v.title);
            if theme.len() >= 4 {
                let c = cookies_owned.clone();
                let q = theme.clone();
            handles.push(s.spawn(move || {
                ("rel_theme".to_string(), youtube::fetch_search(&c, &q, 8))
            }));
            }
            let artist = normalize_uploader(&v.uploader);
            if artist.len() > 3 {
                let c = cookies_owned.clone();
                handles.push(s.spawn(move || {
                    ("rel_uploader".to_string(), youtube::fetch_search(&c, &artist, 6))
                }));
            }
        } else if let Some(last) = history.last_video.as_ref() {
            let theme = simplify_for_search(&last.title);
            if theme.len() >= 4 {
                let c = cookies_owned.clone();
                handles.push(s.spawn(move || {
                    ("rel_last".to_string(), youtube::fetch_search(&c, &theme, 8))
                }));
            }
        }

        if !for_sidebar {
            let explore_n = 2;
            for i in 0..explore_n {
                let idx = (rotation + i) % EXPLORE_QUERIES.len();
                let c = cookies_owned.clone();
                let q = EXPLORE_QUERIES[idx].to_string();
                handles.push(s.spawn(move || {
                    (
                        format!("exp_{i}"),
                        youtube::fetch_search_recent(&c, &q, 5),
                    )
                }));
            }
        }

        for h in handles {
            if let Ok((tag, Ok(items))) = h.join() {
                let kind = if tag == "sub" {
                    SourceKind::Subscription
                } else if tag == "hist" {
                    SourceKind::YtHistory
                } else if tag.starts_with("radio") {
                    SourceKind::Radio
                } else if tag.starts_with("ch_") {
                    SourceKind::ChannelFresh
                } else if tag.starts_with("int_") {
                    SourceKind::InterestFresh
                } else if tag.starts_with("exp_") {
                    SourceKind::Explore
                } else {
                    SourceKind::RelatedSearch
                };
                push_candidates(&mut candidates, items, kind);
            }
        }
    });

    candidates
}

/// Uploads recentes dos canais que voce mais assiste (+ inscricoes com login).
fn fetch_channel_news(cookies: &str, history: &WatchHistory) -> Vec<Video> {
    let has_cookies = !cookies.is_empty();
    let mut uploaders = history.top_uploaders(false, 8);
    let mut batches: Vec<Vec<Video>> = Vec::new();

    if has_cookies {
        if let Ok(subs) = youtube::fetch_subscriptions(cookies, HOME_CHANNEL_LIMIT + 8) {
            for v in &subs {
                let u = normalize_uploader(&v.uploader);
                if u.len() > 2 && !uploaders.iter().any(|x| x.eq_ignore_ascii_case(&u)) {
                    uploaders.push(u);
                }
            }
            if !subs.is_empty() {
                batches.push(subs);
            }
        }
    }

    if uploaders.is_empty() && batches.is_empty() {
        return Vec::new();
    }

    let cookies_owned = cookies.to_string();
    std::thread::scope(|s| {
        let handles: Vec<_> = uploaders
            .iter()
            .take(8)
            .map(|uploader| {
                let c = cookies_owned.clone();
                let q = uploader.clone();
                s.spawn(move || youtube::fetch_search_recent(&c, &q, 5))
            })
            .collect();

        for h in handles {
            if let Ok(Ok(items)) = h.join() {
                if !items.is_empty() {
                    batches.push(items);
                }
            }
        }
    });

    pick_channel_news(batches, HOME_CHANNEL_LIMIT)
}

fn pick_channel_news(batches: Vec<Vec<Video>>, limit: usize) -> Vec<Video> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut per_uploader: HashMap<String, usize> = HashMap::new();
    let mut idx = vec![0usize; batches.len()];

    loop {
        let mut progressed = false;
        for (bi, batch) in batches.iter().enumerate() {
            if out.len() >= limit {
                break;
            }
            while idx[bi] < batch.len() {
                let v = &batch[idx[bi]];
                idx[bi] += 1;
                if !seen.insert(v.id.clone()) {
                    continue;
                }
                let uploader = normalize_uploader(&v.uploader);
                if !uploader.is_empty() {
                    let n = per_uploader.entry(uploader).or_insert(0);
                    if *n >= 2 {
                        continue;
                    }
                    *n += 1;
                }
                out.push(v.clone());
                progressed = true;
                break;
            }
        }
        if out.len() >= limit || !progressed {
            break;
        }
    }

    out
}

pub fn recommend_home_feed(
    cookies: &str,
    seed: Option<Video>,
    history: &WatchHistory,
    rotation: usize,
) -> Result<(Vec<Video>, Vec<Video>, Vec<Video>, Vec<Video>, String), String> {
    let channel_news = fetch_channel_news(cookies, history);
    let channel_ids: HashSet<_> = channel_news.iter().map(|v| v.id.clone()).collect();

    let seed_ref = seed.as_ref().or(history.last_video.as_ref());
    let keywords = history.interest_keywords(false, 8);

    let mut all = collect_candidates(cookies, seed_ref, history, rotation, false);

    let mut seen = channel_ids;
    let mut scored: Vec<(Candidate, f32)> = Vec::new();
    for c in all.drain(..) {
        if c.source == SourceKind::ChannelFresh {
            continue;
        }
        if !seen.insert(c.video.id.clone()) {
            continue;
        }
        let s = score_candidate(&c, history, &keywords);
        scored.push((c, s));
    }

    let feed = pick_ranked(
        scored
            .iter()
            .map(|(c, s)| {
                (
                    Candidate {
                        video: c.video.clone(),
                        source: c.source,
                        rank_in_source: c.rank_in_source,
                    },
                    *s,
                )
            })
            .collect(),
        HOME_FEED_LIMIT,
        2,
    );

    let sidebar_scored: Vec<(Candidate, f32)> = scored
        .into_iter()
        .filter(|(c, _)| {
            matches!(
                c.source,
                SourceKind::Radio | SourceKind::RelatedSearch
            )
        })
        .collect();

    let mut recommended = if !sidebar_scored.is_empty() {
        pick_ranked(sidebar_scored, HOME_REC_LIMIT, 3)
    } else {
        Vec::new()
    };

    if recommended.len() < HOME_REC_LIMIT / 2 {
        if let Some(v) = seed_ref {
            let mut extra = youtube::fetch_rd(cookies, &v.id, HOME_REC_LIMIT)?;
            extra.retain(|x| seen.insert(x.id.clone()));
            recommended.extend(extra);
            recommended.truncate(HOME_REC_LIMIT);
        }
    }

    let mut live = Vec::new();
    let live_queries: Vec<String> = if keywords.is_empty() {
        vec!["ao vivo".into(), "live".into()]
    } else {
        keywords
            .iter()
            .take(2)
            .map(|k| format!("{k} ao vivo"))
            .collect()
    };
    for q in live_queries {
        if let Ok(mut items) = youtube::fetch_search(cookies, &q, HOME_LIVE_LIMIT * 2) {
            items.retain(|v| v.is_live);
            for (i, v) in items.into_iter().enumerate() {
                if live.len() >= HOME_LIVE_LIMIT {
                    break;
                }
                if seen.insert(v.id.clone()) {
                    let _ = i;
                    live.push(v);
                }
            }
        }
    }

    let seed_label = seed_ref
        .map(|v| format!("continuar: {}", v.title))
        .unwrap_or_else(|| {
            if !cookies.is_empty() {
                "para voce · inscricoes · novidades".into()
            } else if !keywords.is_empty() {
                format!("para voce · {}", keywords.join(", "))
            } else {
                "para voce · explorar".into()
            }
        });

    Ok((feed, recommended, live, channel_news, seed_label))
}

/// Candidatos para recarga da fila (estilo autoplay do YouTube).
pub fn queue_refill_queries(
    last_search: &str,
    seed: &Video,
    rotation: usize,
    history: &WatchHistory,
) -> Vec<String> {
    let mut out = Vec::new();

    let theme = simplify_for_search(&seed.title);
    if theme.len() >= 4 {
        out.push(theme);
    }

    let artist = normalize_uploader(&seed.uploader);
    if artist.len() > 3 {
        out.push(artist);
    }

    for kw in history.interest_keywords(false, 3) {
        out.push(kw);
    }

    let ls = last_search.trim();
    if !ls.is_empty() && ls != seed.id {
        out.push(ls.to_string());
    }

    let idx = (rotation + 1) % EXPLORE_QUERIES.len();
    out.push(EXPLORE_QUERIES[idx].to_string());

    out.sort();
    out.dedup();
    out.truncate(6);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_bias_is_soft() {
        assert!(left_bias_score("marxismo para iniciantes") > 0.0);
        assert_eq!(left_bias_score("receita de bolo"), 0.0);
    }

    #[test]
    fn explore_cap_limits_left_pool() {
        let mut scored = Vec::new();
        for i in 0..20 {
            scored.push((
                Candidate {
                    video: Video {
                        id: format!("e{i}"),
                        title: "marxismo".into(),
                        uploader: format!("ch{i}"),
                        duration: "10:00".into(),
                        url: String::new(),
                        thumbnail: String::new(),
                        is_live: false,
                    },
                    source: SourceKind::Explore,
                    rank_in_source: 0,
                },
                0.9,
            ));
        }
        let picked = pick_ranked(scored, 12, 2);
        assert!(picked.len() <= 12);
    }
}
