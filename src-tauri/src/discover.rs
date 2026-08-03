//! Busca de faixas para recarga da fila com variedade (genero/estilo, nao so mesmo artista).

use crate::youtube::Video;
use std::collections::{HashMap, HashSet};

struct GenreProfile {
    triggers: &'static [&'static str],
    styles: &'static [&'static str],
}

const GENRE_PROFILES: &[GenreProfile] = &[
    GenreProfile {
        triggers: &["sertanejo", "modao", "modão", "sofrencia", "sofrência", "arrocha", "renner", "dupla sertaneja"],
        styles: &[
            "sertanejo universitario",
            "modao sertanejo",
            "sertanejo romantico",
            "sertanejo sofrencia",
            "sertanejo raiz",
            "sertanejo modao",
            "sertanejo arrocha",
            "modao raiz",
        ],
    },
    GenreProfile {
        triggers: &["forro", "forró", "piseiro", "xote", "brega"],
        styles: &[
            "forro piseiro",
            "forro romantico",
            "forro pe de serra",
            "piseiro 2024",
            "forro eletronico",
            "brega funk",
        ],
    },
    GenreProfile {
        triggers: &["pagode", "samba"],
        styles: &[
            "pagode romantico",
            "pagode anos 90",
            "samba pagode",
            "pagode raiz",
            "samba enredo",
            "pagode de mesa",
        ],
    },
    GenreProfile {
        triggers: &["funk", "funk carioca", "beat"],
        styles: &[
            "funk carioca",
            "funk melody",
            "funk 150 bpm",
            "funk consciente",
            "funk rave",
            "funk ostentacao",
        ],
    },
    GenreProfile {
        triggers: &["mpb", "bossa", "bossa nova"],
        styles: &[
            "mpb classica",
            "mpb contemporanea",
            "bossa nova",
            "mpb anos 70",
            "mpb acustico",
        ],
    },
    GenreProfile {
        triggers: &["rock", "metal", "punk"],
        styles: &[
            "rock nacional",
            "rock alternativo",
            "rock brasileiro",
            "metal nacional",
            "punk rock brasil",
        ],
    },
    GenreProfile {
        triggers: &["gospel", "louvor", "adoracao", "adoração", "ccr"],
        styles: &[
            "louvor adoracao",
            "gospel brasileiro",
            "musica gospel",
            "worship brasil",
            "gospel contemporaneo",
        ],
    },
    GenreProfile {
        triggers: &["axe", "axé", "reggae", "reggae br"],
        styles: &[
            "axe anos 90",
            "axé music",
            "reggae brasil",
            "reggae roots br",
            "samba reggae",
        ],
    },
    GenreProfile {
        triggers: &["trap", "drill", "rap", "hip hop"],
        styles: &[
            "trap brasil",
            "rap nacional",
            "drill brasil",
            "hip hop br",
            "rap consciente",
        ],
    },
    GenreProfile {
        triggers: &["eletronica", "eletrônica", "house", "techno", "edm"],
        styles: &[
            "eletronica brasileira",
            "house brasil",
            "techno br",
            "deep house br",
            "edm festival",
        ],
    },
    GenreProfile {
        triggers: &["country", "pop", "indie", "acustico", "acústico"],
        styles: &[
            "country americano",
            "pop internacional",
            "indie rock",
            "musica acustica",
            "pop acustico",
        ],
    },
];

pub fn simplify_for_search(title: &str) -> String {
    let lower = title.to_lowercase();
    let noise = [
        "official video",
        "official music video",
        "video oficial",
        "clipe oficial",
        "lyrics",
        "legendado",
        "ao vivo",
        "live",
        "hd",
        "4k",
        "ft.",
        "feat.",
        "audio oficial",
        "videoclipe oficial",
    ];
    let mut s = lower;
    for n in noise {
        s = s.replace(n, " ");
    }
    s.split(|c: char| !c.is_alphanumeric() && c != ' ')
        .filter(|w| w.len() > 2)
        .take(6)
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn normalize_uploader(name: &str) -> String {
    name.to_lowercase()
        .split('|')
        .next()
        .unwrap_or(name)
        .trim()
        .to_string()
}

/// Chave canonica do artista (titulo > uploader generico).
pub fn artist_key(v: &Video) -> String {
    normalize_uploader(&extract_artist_label(v))
}

fn parse_duration_secs(d: &str) -> Option<u32> {
    let parts: Vec<u32> = d.split(':').filter_map(|p| p.parse().ok()).collect();
    match parts.as_slice() {
        [m, s] => Some(m * 60 + s),
        [h, m, s] => Some(h * 3600 + m * 60 + s),
        _ => None,
    }
}

/// Faixa tocavel — exclui compilacoes, DVDs e lives.
pub fn is_playable_track(v: &Video) -> bool {
    if v.is_live {
        return false;
    }
    let title = v.title.to_lowercase();
    let noisy = [
        "dvd",
        "as melhores",
        "melhores musicas",
        "pot pourri",
        "pot-pourri",
        "full album",
        "album completo",
        "complete album",
        "ao vivo no",
        "show completo",
        "megamix",
        "non-stop",
    ];
    if noisy.iter().any(|n| title.contains(n)) {
        return false;
    }
    if let Some(secs) = parse_duration_secs(&v.duration) {
        if secs > 600 {
            return false;
        }
    }
    true
}

pub fn filter_playable(videos: Vec<Video>) -> Vec<Video> {
    videos.into_iter().filter(|v| is_playable_track(v)).collect()
}

fn normalize_text(s: &str) -> String {
    s.to_lowercase()
        .replace('ã', "a")
        .replace('á', "a")
        .replace('â', "a")
        .replace('à', "a")
        .replace('é', "e")
        .replace('ê', "e")
        .replace('í', "i")
        .replace('ó', "o")
        .replace('ô', "o")
        .replace('õ', "o")
        .replace('ú', "u")
        .replace('ç', "c")
}

fn matched_profiles(context: &str) -> Vec<&'static GenreProfile> {
    let norm = normalize_text(context);
    GENRE_PROFILES
        .iter()
        .filter(|p| p.triggers.iter().any(|t| norm.contains(&normalize_text(t))))
        .collect()
}

fn rotate_pick(items: &[String], rotation: usize, count: usize) -> Vec<String> {
    if items.is_empty() {
        return Vec::new();
    }
    let start = rotation % items.len();
    let mut out = Vec::with_capacity(count);
    for i in 0..items.len() {
        if out.len() >= count {
            break;
        }
        out.push(items[(start + i) % items.len()].clone());
    }
    out
}

fn expand_style_queries(context: &str, rotation: usize) -> Vec<String> {
    let profiles = matched_profiles(context);
    if profiles.is_empty() {
        return Vec::new();
    }

    let mut styles: Vec<String> = profiles
        .iter()
        .flat_map(|p| p.styles.iter().map(|s| (*s).to_string()))
        .collect();
    styles.sort();
    styles.dedup();
    rotate_pick(&styles, rotation, 5)
}

/// Intercala listas para misturar fontes (genero, mix, busca).
pub fn interleave_sources(sources: Vec<Vec<Video>>) -> Vec<Video> {
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

/// Evita repetir artista/faixa — max 1 por artista por lote.
pub fn pick_diverse_candidates(
    candidates: Vec<Video>,
    exclude_ids: &HashSet<String>,
    queue_counts: &HashMap<String, usize>,
    seed_artist: &str,
    limit: usize,
) -> Vec<Video> {
    let playable: Vec<Video> = filter_playable(candidates);
    let queue_len = queue_counts.values().sum::<usize>().max(1);
    let mut batch_counts: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::new();
    let seed_key = normalize_uploader(seed_artist);

    for v in playable {
        if exclude_ids.contains(&v.id) {
            continue;
        }
        let artist = artist_key(&v);
        if artist.is_empty() {
            out.push(v);
            if out.len() >= limit {
                break;
            }
            continue;
        }

        let in_queue = queue_counts.get(&artist).copied().unwrap_or(0);
        let in_batch = batch_counts.get(&artist).copied().unwrap_or(0);
        let max_batch = if !seed_key.is_empty() && artist == seed_key {
            1
        } else {
            1
        };
        let queue_share = in_queue as f32 / queue_len as f32;
        if queue_share > 0.25 && in_batch >= 1 {
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

/// Novos artistas: prioriza uploaders que nao estao no historico.
pub fn pick_new_artists(
    candidates: Vec<Video>,
    exclude_ids: &HashSet<String>,
    known_uploaders: &HashSet<String>,
    limit: usize,
) -> Vec<Video> {
    let mut batch_counts: HashMap<String, usize> = HashMap::new();
    let mut fresh = Vec::new();
    let mut fallback = Vec::new();

    for v in filter_playable(candidates) {
        if exclude_ids.contains(&v.id) {
            continue;
        }
        let artist = artist_key(&v);
        if artist.is_empty() {
            fallback.push(v);
            continue;
        }
        if batch_counts.get(&artist).copied().unwrap_or(0) >= 1 {
            continue;
        }
        batch_counts.insert(artist.clone(), 1);
        if known_uploaders.contains(&artist) {
            fallback.push(v);
        } else {
            fresh.push(v);
        }
    }

    let mut out = fresh;
    if out.len() < limit {
        out.extend(fallback.into_iter().take(limit - out.len()));
    }
    out.truncate(limit);
    out
}

const NOISE_BLOCK: &[&str] = &[
    "rick and morty",
    "rick e morty",
    "rick & morty",
    "tipo rick",
    "rock classic",
    "rock 2000",
    "best of rock",
    "live timelapse",
    "aparições",
    "aparicoes",
    "cinematográfica",
    "cinematografica",
    "react ",
    "podcast",
    "#shorts",
    "mc rick",
];

const GENERIC_UPLOADERS: &[&str] = &[
    "music brasil",
    "vevo",
    "official",
    "oficial",
    "topic",
    "records",
    "som livre",
    "warner",
    "sony",
];

pub fn extract_artist_label(v: &Video) -> String {
    let title_artist = v
        .title
        .split_once(" - ")
        .or_else(|| v.title.split_once(" | "))
        .map(|(a, _)| a.trim())
        .filter(|a| a.len() > 2);

    let uploader = v
        .uploader
        .split('|')
        .next()
        .unwrap_or(&v.uploader)
        .trim();
    let up_low = uploader.to_lowercase();
    let generic = GENERIC_UPLOADERS.iter().any(|g| up_low.contains(g));

    if generic {
        if let Some(a) = title_artist {
            return a.to_string();
        }
    }
    if let Some(a) = title_artist {
        if a.len() >= uploader.len() || generic {
            return a.to_string();
        }
    }
    uploader.to_string()
}

#[derive(Clone, Debug)]
pub struct MusicContext {
    pub artist_label: String,
    pub artist_norm: String,
    pub genre_styles: Vec<String>,
    pub block_terms: Vec<String>,
}

pub fn build_music_context(last_search: &str, seed: &Video) -> MusicContext {
    let artist_label = extract_artist_label(seed);
    let artist_norm = normalize_uploader(&artist_label);
    let rich = format!("{} {} {}", last_search, seed.title, artist_label);
    let mut genre_styles = expand_style_queries(&rich, 0);
    if genre_styles.is_empty() && infer_sertanejo(seed) {
        genre_styles = vec![
            "sertanejo romantico".into(),
            "sertanejo universitario".into(),
            "dupla sertaneja".into(),
            "modao sertanejo".into(),
        ];
    }
    let mut block_terms: Vec<String> = NOISE_BLOCK.iter().map(|s| s.to_string()).collect();
    let ls = last_search.trim().to_lowercase();
    if !ls.is_empty() && ls.len() <= 5 {
        block_terms.push("rock ".into());
        block_terms.push("⚡".into());
        block_terms.push("tipo rick".into());
    }
    MusicContext {
        artist_label,
        artist_norm,
        genre_styles,
        block_terms,
    }
}

pub fn infer_sertanejo(v: &Video) -> bool {
    let blob = format!("{} {}", v.title, v.uploader).to_lowercase();
    blob.contains("sertanejo")
        || blob.contains("renner")
        || blob.contains("modao")
        || blob.contains("modão")
        || blob.contains("di camargo")
        || blob.contains("zeze")
        || blob.contains("chitãozinho")
        || blob.contains("chitozinho")
        || blob.contains("xororó")
        || blob.contains("xororo")
        || blob.contains("leandro e leonardo")
        || (blob.contains("rick") && blob.contains('&'))
}

const SERTANEJO_PEERS: &[&str] = &[
    "bruno e marrone",
    "edson e hudson",
    "eduardo costa",
    "zeze di camargo",
    "chitãozinho e xororó",
    "leandro e leonardo",
    "fernando e sorocaba",
    "jorge e mateus",
    "henrique e juliano",
    "gusttavo lima",
    "luan santana",
    "michel telo",
    "daniel",
    "victor e leo",
];

const GLOBAL_COLD: &[&str] = &[
    "top hits 2024",
    "pop hits mix",
    "hip hop mix",
    "rock hits playlist",
    "r&b soul mix",
    "electronic dance mix",
    "indie alternative mix",
    "latin hits 2024",
    "chill music mix",
    "new music friday",
];

const BR_COLD: &[&str] = &[
    "sertanejo romantico 2024",
    "pagode anos 90",
    "forro piseiro",
    "mpb brasileira",
    "sertanejo universitario",
    "funk melody",
    "axe music",
];

fn artists_match(a: &str, b: &str) -> bool {
    let a = normalize_uploader(a);
    let b = normalize_uploader(b);
    a == b || a.contains(&b) || b.contains(&a)
}

pub fn peer_artists(seed: &Video, rotation: usize, count: usize) -> Vec<String> {
    let current = extract_artist_label(seed);
    let mut peers = Vec::new();

    if infer_sertanejo(seed) {
        for p in SERTANEJO_PEERS {
            if !artists_match(&current, p) {
                peers.push((*p).to_string());
            }
        }
    } else {
        let ctx = build_music_context("", seed);
        for style in &ctx.genre_styles {
            peers.push(format!("{style} artista"));
        }
    }

    rotate_pick(&peers, rotation, count)
}

pub fn contextual_search_queries(last_search: &str, seed: &Video, rotation: usize) -> Vec<String> {
    let ctx = build_music_context(last_search, seed);
    let mut out = Vec::new();

    out.extend(peer_artists(seed, rotation, 6));
    out.extend(rotate_pick(&ctx.genre_styles, rotation, 4));

    if ctx.artist_label.len() > 3 {
        out.push(format!("{} musica", ctx.artist_label));
    }

    let ls = last_search.trim();
    if ls.len() >= 6 {
        out.push(format!("{ls} musica"));
    }

    out.sort();
    out.dedup();
    out
}

pub fn cold_start_queries(prefer_brazilian: bool, rotation: usize) -> Vec<String> {
    let pool: Vec<String> = if prefer_brazilian {
        BR_COLD.iter().map(|s| (*s).to_string()).collect()
    } else {
        GLOBAL_COLD.iter().map(|s| (*s).to_string()).collect()
    };
    rotate_pick(&pool, rotation, 5)
}

pub fn is_relevant(ctx: &MusicContext, v: &Video) -> bool {
    if v.is_live {
        return false;
    }
    let blob = format!("{} {}", v.title, v.uploader).to_lowercase();
    for block in &ctx.block_terms {
        if blob.contains(block) {
            return false;
        }
    }
    if ctx.genre_styles.iter().any(|s| s.contains("sertanejo")) {
        if blob.contains("rock classic") || blob.contains("rock 2000") || blob.contains("⚡") {
            return false;
        }
        if (blob.contains("funk") || blob.contains("mc rick")) && !blob.contains("sertanejo") {
            return false;
        }
    }
    if blob.contains("aparições") || blob.contains("aparicoes") || blob.contains("cinematográfica") {
        return false;
    }
    true
}

pub fn relevance_score(ctx: &MusicContext, v: &Video) -> i32 {
    let blob = format!("{} {}", v.title, v.uploader).to_lowercase();
    let mut score = 0;
    if !ctx.artist_norm.is_empty() && blob.contains(&ctx.artist_norm) {
        score += 20;
    }
    let label = ctx.artist_label.to_lowercase();
    if !label.is_empty() && blob.contains(&label) {
        score += 15;
    }
    for g in &ctx.genre_styles {
        if blob.contains(g) {
            score += 5;
        }
    }
    if infer_sertanejo(v) && ctx.genre_styles.iter().any(|s| s.contains("sertanejo")) {
        score += 8;
    }
    score
}

pub fn filter_relevant(ctx: &MusicContext, videos: Vec<Video>) -> Vec<Video> {
    videos
        .into_iter()
        .filter(|v| is_playable_track(v) && is_relevant(ctx, v))
        .collect()
}

pub fn filter_and_rank(ctx: &MusicContext, videos: Vec<Video>) -> Vec<Video> {
    let mut out: Vec<Video> = videos
        .into_iter()
        .filter(|v| is_playable_track(v) && is_relevant(ctx, v))
        .collect();
    out.sort_by(|a, b| relevance_score(ctx, b).cmp(&relevance_score(ctx, a)));
    out
}

pub fn refine_search_results(query: &str, items: Vec<Video>) -> Vec<Video> {
    if items.is_empty() {
        return items;
    }
    let q = query.trim();
    let seed = items
        .iter()
        .find(|v| infer_sertanejo(v))
        .or_else(|| items.iter().find(|v| !v.is_live && v.duration.contains(':')))
        .cloned()
        .unwrap_or_else(|| items[0].clone());
    let ctx = build_music_context(q, &seed);
    if q.len() <= 6 || !q.contains(' ') {
        filter_and_rank(&ctx, items)
    } else {
        filter_relevant(&ctx, items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(title: &str) -> Video {
        Video {
            id: "abc".into(),
            title: title.into(),
            uploader: "Zeze Di Camargo".into(),
            duration: "200".into(),
            url: String::new(),
            thumbnail: String::new(),
            is_live: false,
        }
    }

    #[test]
    fn contextual_includes_music_queries() {
        let q = contextual_search_queries("sertanejo romantico", &seed("Amor Covarde"), 0);
        assert!(!q.is_empty());
        assert!(q.iter().any(|s| s.contains("musica")));
    }

    #[test]
    fn contextual_rotates() {
        let a = contextual_search_queries("sertanejo", &seed("Teste"), 0);
        let b = contextual_search_queries("sertanejo", &seed("Teste"), 3);
        assert_ne!(a, b);
    }
}
