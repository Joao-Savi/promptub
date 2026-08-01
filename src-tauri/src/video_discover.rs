//! Descoberta de videos com vies comunista (nao so politica).

use crate::discover::{normalize_uploader, simplify_for_search};
use crate::history::WatchHistory;
use crate::youtube::Video;

struct TopicPool {
    topics: &'static [&'static str],
}

const POOLS: &[TopicPool] = &[
    TopicPool {
        topics: &[
            "marxismo explicado",
            "leninismo",
            "materialismo historico",
            "dialética materialista",
            "manifesto comunista",
            "estado e revolucao lenin",
        ],
    },
    TopicPool {
        topics: &[
            "capital karl marx resumo",
            "critica ao capitalismo",
            "mais valia explicada",
            "imperialismo lenin",
            "economia politica marxista",
        ],
    },
    TopicPool {
        topics: &[
            "cinema sovietico",
            "literatura marxista",
            "arte proletaria",
            "musica protesto trabalhador",
            "cultura popular esquerda",
            "humor critica capitalismo",
        ],
    },
    TopicPool {
        topics: &[
            "historia movimento operario",
            "revolucao proletaria",
            "partido comunista historia",
            "historia sindical brasil",
            "socialismo real existente",
        ],
    },
    TopicPool {
        topics: &[
            "analise filme marxista",
            "video game critica capitalismo",
            "ciencia popular marxismo",
            "documentario trabalhadores",
            "filosofia materialismo",
            "sociedade classe media",
        ],
    },
    TopicPool {
        topics: &[
            "comunismo brasil historia",
            "movimento operario brasil",
            "esquerda brasil documentario",
            "cultura popular brasileira",
        ],
    },
];

const RELATED_SUFFIXES: &[&str] = &[
    "marxismo",
    "analise marxista",
    "materialismo historico",
    "trabalhadores",
    "socialismo",
    "critica capitalismo",
];

fn rotate_pick(items: &[&str], rotation: usize, count: usize) -> Vec<String> {
    if items.is_empty() {
        return Vec::new();
    }
    let start = rotation % items.len();
    let mut out = Vec::with_capacity(count);
    for i in 0..items.len() {
        if out.len() >= count {
            break;
        }
        out.push(items[(start + i) % items.len()].to_string());
    }
    out
}

fn history_queries(history: &WatchHistory, rotation: usize) -> Vec<String> {
    let mut out = Vec::new();

    if let Some(v) = history.last_video.as_ref() {
        let theme = simplify_for_search(&v.title);
        if theme.len() >= 4 {
            let suffix = RELATED_SUFFIXES[rotation % RELATED_SUFFIXES.len()];
            out.push(format!("{theme} {suffix}"));
        }
        if !v.uploader.is_empty() {
            out.push(format!("{} videos", normalize_uploader(&v.uploader)));
        }
    }

    for uploader in history.top_uploaders(false, 2) {
        out.push(uploader);
    }

    out.sort();
    out.dedup();
    out
}

/// Feed diversificado: teoria + cultura + historia + cotidiano (vies comunista).
pub fn video_feed_queries(rotation: usize, history: Option<&WatchHistory>) -> Vec<String> {
    let mut out = Vec::new();

    for (i, pool) in POOLS.iter().enumerate() {
        out.extend(rotate_pick(pool.topics, rotation + i * 2, 1));
    }

    if let Some(h) = history {
        out.extend(history_queries(h, rotation));
    }

    out.sort();
    out.dedup();
    out.truncate(6);
    out
}

pub fn video_related_queries(
    last_search: &str,
    seed: &Video,
    rotation: usize,
    history: Option<&WatchHistory>,
) -> Vec<String> {
    let mut out = video_feed_queries(rotation + 1, history);
    out.truncate(3);

    let ls = last_search.trim();
    if !ls.is_empty() {
        out.push(format!("{ls} analise marxista"));
    }

    let theme = simplify_for_search(&seed.title);
    let artist = normalize_uploader(&seed.uploader);
    let theme_words: Vec<&str> = theme
        .split_whitespace()
        .filter(|w| {
            w.len() > 2
                && !artist.contains(w)
                && !artist.split_whitespace().any(|a| a == *w)
        })
        .collect();

    if theme_words.len() >= 2 {
        let theme_only = theme_words.join(" ");
        let suffix = RELATED_SUFFIXES[rotation % RELATED_SUFFIXES.len()];
        out.push(format!("{theme_only} {suffix}"));
    }

    if !artist.is_empty() && artist.len() > 3 {
        out.push(artist);
    }

    out.sort();
    out.dedup();
    out
}

pub fn video_live_queries(rotation: usize) -> Vec<String> {
    rotate_pick(
        &[
            "live marxismo",
            "live cultura esquerda",
            "live sindicato",
            "live documentario",
        ],
        rotation,
        2,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feed_covers_multiple_pools() {
        let q = video_feed_queries(0, None);
        assert!(q.len() >= 4);
    }
}
