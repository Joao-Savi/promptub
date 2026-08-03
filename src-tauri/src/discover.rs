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

/// Evita repetir artista/faixa — max 1 por artista por lote, com score de gosto.
pub fn pick_diverse_candidates(
    candidates: Vec<Video>,
    exclude_ids: &HashSet<String>,
    exclude_fingerprints: &HashSet<String>,
    queue_counts: &HashMap<String, usize>,
    seed_artist: &str,
    limit: usize,
    ctx: Option<&MusicContext>,
    taste: Option<&crate::history::TasteProfile>,
) -> Vec<Video> {
    let playable: Vec<Video> = filter_playable(candidates);
    let queue_len = queue_counts.values().sum::<usize>().max(1);
    let seed_key = normalize_uploader(seed_artist);

    let mut scored: Vec<(i32, Video)> = Vec::new();
    let mut batch_artists: HashMap<String, usize> = HashMap::new();

    for v in playable {
        if exclude_ids.contains(&v.id) {
            continue;
        }
        let fp = title_fingerprint(&v);
        if exclude_fingerprints.contains(&fp) {
            continue;
        }

        if let Some(t) = taste {
            if t.is_blocked(&v) {
                continue;
            }
        }

        if let Some(c) = ctx {
            if !is_relevant(c, &v) || has_cross_genre_conflict(c, &v) {
                continue;
            }
        }

        let artist = artist_key(&v);
        if !artist.is_empty() {
            let in_batch = batch_artists.get(&artist).copied().unwrap_or(0);
            if in_batch >= 1 {
                continue;
            }
            let in_queue = queue_counts.get(&artist).copied().unwrap_or(0);
            let queue_share = in_queue as f32 / queue_len as f32;
            if queue_share > 0.2 && artist != seed_key {
                continue;
            }
            if !seed_key.is_empty() && artist == seed_key {
                continue;
            }
        }

        let mut score = ctx.map(|c| relevance_score(c, &v)).unwrap_or(0);
        if let Some(t) = taste {
            let st = t.state_for(&v);
            match st {
                crate::history::TasteState::Liked => score += 40,
                crate::history::TasteState::Disliked => continue,
                crate::history::TasteState::None => {}
            }
        }

        scored.push((score, v));
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0));

    let mut out = Vec::new();
    for (_, v) in scored {
        let artist = artist_key(&v);
        if !artist.is_empty() {
            let in_batch = batch_artists.get(&artist).copied().unwrap_or(0);
            if in_batch >= 1 {
                continue;
            }
            batch_artists.insert(artist, 1);
        }
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
    exclude_fingerprints: &HashSet<String>,
    known_uploaders: &HashSet<String>,
    taste: Option<&crate::history::TasteProfile>,
    limit: usize,
) -> Vec<Video> {
    let mut batch_counts: HashMap<String, usize> = HashMap::new();
    let mut fresh = Vec::new();
    let mut fallback = Vec::new();

    for v in filter_playable(candidates) {
        if exclude_ids.contains(&v.id) {
            continue;
        }
        if exclude_fingerprints.contains(&title_fingerprint(&v)) {
            continue;
        }
        if let Some(t) = taste {
            if t.is_blocked(&v) {
                continue;
            }
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

const NON_MUSIC_BLOCK: &[&str] = &[
    "documentario",
    "documentário",
    "documentary",
    "entrevista",
    "interview",
    "tutorial",
    "review",
    "trailer",
    "filme",
    "movie",
    "episodio",
    "episódio",
    "episode",
    "making of",
    "behind the scenes",
    "curiosidades",
    "historia de",
    "história de",
    "biografia",
    "aula",
    "lecture",
    "palestra",
    "debate",
    "noticias",
    "notícias",
    "news ",
    "gameplay",
    "walkthrough",
    "vlog",
    "unboxing",
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

pub fn title_fingerprint(v: &Video) -> String {
    let artist = extract_artist_label(v);
    let title_part = v
        .title
        .split_once(" - ")
        .or_else(|| v.title.split_once(" | "))
        .map(|(_, t)| t.trim())
        .unwrap_or(v.title.trim());
    let simplified = simplify_for_search(title_part);
    format!("{}::{}", normalize_uploader(&artist), simplified)
}

pub fn build_music_context_rich(rich_context: &str, seed: &Video) -> MusicContext {
    let artist_label = extract_artist_label(seed);
    let artist_norm = normalize_uploader(&artist_label);
    let rich = format!("{rich_context} {} {artist_label}", seed.title);
    let mut genre_styles = expand_style_queries(&rich, 0);
    if genre_styles.is_empty() && infer_sertanejo(seed) {
        genre_styles = vec![
            "sertanejo romantico".into(),
            "sertanejo universitario".into(),
            "dupla sertaneja".into(),
            "modao sertanejo".into(),
        ];
    }
    let mut block_terms: Vec<String> = NOISE_BLOCK
        .iter()
        .chain(NON_MUSIC_BLOCK.iter())
        .map(|s| s.to_string())
        .collect();
    let ls = rich_context.trim().to_lowercase();
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

pub fn build_music_context(last_search: &str, seed: &Video) -> MusicContext {
    build_music_context_rich(last_search, seed)
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

fn primary_genre_triggers(context: &str) -> HashSet<String> {
    let profiles = matched_profiles(context);
    let mut triggers = HashSet::new();
    for p in profiles.iter().take(2) {
        for t in p.triggers {
            triggers.insert(normalize_text(t));
        }
    }
    triggers
}

/// Bloqueia faixas de genero diferente (ex.: rock no meio do sertanejo).
pub fn has_cross_genre_conflict(ctx: &MusicContext, v: &Video) -> bool {
    if ctx.genre_styles.is_empty() {
        return false;
    }
    let rich = format!(
        "{} {} {}",
        ctx.artist_label,
        ctx.genre_styles.join(" "),
        ctx.artist_norm
    );
    let primary = primary_genre_triggers(&rich);
    if primary.is_empty() {
        return false;
    }

    let blob = normalize_text(&format!("{} {}", v.title, v.uploader));
    for profile in GENRE_PROFILES {
        let video_matches = profile
            .triggers
            .iter()
            .any(|t| blob.contains(&normalize_text(t)));
        if !video_matches {
            continue;
        }
        let is_primary = profile
            .triggers
            .iter()
            .any(|t| primary.contains(&normalize_text(t)));
        if !is_primary {
            return true;
        }
    }
    false
}

fn query_duplicates_seed(query: &str, seed: &Video) -> bool {
    let q = normalize_text(query);
    let artist = normalize_text(&extract_artist_label(seed));
    let title = normalize_text(&seed.title);
    if artist.len() > 3 && q.contains(&artist) {
        return true;
    }
    if title.len() > 5 && q.contains(&simplify_for_search(&title)) {
        return true;
    }
    false
}

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

    for peer in peer_artists(seed, rotation, 8) {
        out.push(format!("{peer} musica"));
    }
    for style in rotate_pick(&ctx.genre_styles, rotation, 5) {
        out.push(format!("{style} musica"));
    }

    let ls = last_search.trim();
    if ls.len() >= 8 && !query_duplicates_seed(ls, seed) {
        out.push(format!("{ls} musica"));
    }

    out.retain(|q| !query_duplicates_seed(q, seed));
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
    for block in NON_MUSIC_BLOCK {
        if blob.contains(block) {
            return false;
        }
    }
    if has_cross_genre_conflict(ctx, v) {
        return false;
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
