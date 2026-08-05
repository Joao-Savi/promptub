use crate::deps::{find_ytdlp, utf8_cmd};
use crate::discover::extract_artist_label;
use crate::text::decode_bytes;
use crate::youtube::Video;
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const LRCLIB_STRONG_SCORE: f64 = 0.75;
const LRCLIB_MIN_SCORE: f64 = 0.65;
const LRCLIB_WAIT: Duration = Duration::from_millis(3000);

struct PartialLyrics {
    lrclib: Option<(Vec<LyricLine>, f64)>,
    youtube: Option<Vec<LyricLine>>,
}

#[derive(Clone, Serialize)]
pub struct LyricLine {
    pub start: f64,
    pub end: f64,
    pub text: String,
    /// Timestamps reais (LRC/legendas). False = estimativa, nao usar lead agressivo.
    #[serde(default = "default_synced")]
    pub synced: bool,
    /// Origem: lrclib, youtube ou plain — frontend aplica lag fino por fonte.
    #[serde(default = "default_source")]
    pub source: String,
}

fn default_synced() -> bool {
    true
}

fn default_source() -> String {
    "lrclib".into()
}

fn tag_source(mut lines: Vec<LyricLine>, source: &str) -> Vec<LyricLine> {
    for line in &mut lines {
        line.source = source.to_string();
    }
    lines
}

pub fn lookup_lyrics(cookies: &str, video_id: &str, title: &str, artist: &str) -> Result<Vec<LyricLine>, String> {
    let id = video_id.trim();
    if id.is_empty() {
        return Err("ID invalido".into());
    }

    let meta = parse_lyrics_meta(title, artist);
    let partial = Arc::new(Mutex::new(PartialLyrics {
        lrclib: None,
        youtube: None,
    }));

    let p_lrclib = Arc::clone(&partial);
    let meta_lrclib = meta.clone();
    let h_lrclib = thread::spawn(move || {
        if let Ok((lines, score)) = fetch_lrclib_strict_scored(&meta_lrclib) {
            if let Ok(clean) = sanitize_lyrics(lines) {
                let mut guard = p_lrclib.lock().unwrap_or_else(|e| e.into_inner());
                guard.lrclib = Some((tag_source(clean, "lrclib"), score));
            }
        }
    });

    let p_yt = Arc::clone(&partial);
    let cookies = cookies.to_string();
    let id_owned = id.to_string();
    let h_yt = thread::spawn(move || {
        if let Ok(lines) = fetch_youtube_subs(&cookies, &id_owned) {
            if let Ok(clean) = sanitize_lyrics(lines) {
                let mut guard = p_yt.lock().unwrap_or_else(|e| e.into_inner());
                guard.youtube = Some(tag_source(clean, "youtube"));
            }
        }
    });

    let started = Instant::now();
    while started.elapsed() < LRCLIB_WAIT {
        let guard = partial.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((lines, score)) = guard.lrclib.clone() {
            if score >= LRCLIB_STRONG_SCORE {
                return Ok(lines);
            }
        }
        if guard.lrclib.is_some() && guard.youtube.is_some() {
            break;
        }
        drop(guard);
        thread::sleep(Duration::from_millis(40));
    }

    h_lrclib.join().ok();
    h_yt.join().ok();

    let guard = partial.lock().unwrap_or_else(|e| e.into_inner());
    pick_lyrics_result(guard.lrclib.clone(), guard.youtube.clone())
}

fn pick_lyrics_result(
    lrclib: Option<(Vec<LyricLine>, f64)>,
    youtube: Option<Vec<LyricLine>>,
) -> Result<Vec<LyricLine>, String> {
    match (lrclib, youtube) {
        (Some((_lrc, score)), Some(yt)) if score < LRCLIB_STRONG_SCORE => Ok(yt),
        (Some((lrc, score)), _) if score >= LRCLIB_MIN_SCORE => Ok(lrc),
        (None, Some(yt)) => Ok(yt),
        _ => Err("Letra sincronizada nao encontrada para esta faixa".into()),
    }
}

#[derive(Clone)]
struct LyricsMeta {
    artist: String,
    track_short: String,
    track_full: String,
    search_query: String,
}

fn parse_lyrics_meta(title: &str, uploader: &str) -> LyricsMeta {
    let video = Video {
        id: String::new(),
        title: title.trim().to_string(),
        uploader: uploader.trim().to_string(),
        duration: String::new(),
        url: String::new(),
        thumbnail: String::new(),
        is_live: false,
    };

    let artist = extract_artist_label(&video);
    let after_sep = title
        .trim()
        .split_once(" - ")
        .or_else(|| title.trim().split_once(" | "))
        .map(|(_, rest)| rest.trim())
        .unwrap_or(title.trim());

    let track_full = strip_bracket_tags(after_sep);
    let track_short = track_full
        .split_once('(')
        .map(|(short, _)| short.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(track_full.as_str())
        .to_string();

    let search_query = if !artist.is_empty() {
        format!("{artist} {track_short}")
    } else {
        track_short.clone()
    };

    LyricsMeta {
        artist,
        track_short,
        track_full,
        search_query,
    }
}

fn strip_bracket_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '[' => in_tag = true,
            ']' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn fetch_lrclib_strict_scored(meta: &LyricsMeta) -> Result<(Vec<LyricLine>, f64), String> {
    let mut best: Option<(f64, Vec<LyricLine>)> = None;

    if !meta.artist.is_empty() {
        for track in [&meta.track_short, &meta.track_full] {
            if track.is_empty() {
                continue;
            }
            if let Ok((lines, score)) = lrclib_get_validated_scored(track, &meta.artist, meta) {
                upsert_lrclib_best(&mut best, score, lines);
            }
        }
    }

    for query in [&meta.search_query, &format!("{} {}", meta.artist, meta.track_short)] {
        if query.trim().is_empty() {
            continue;
        }
        if let Ok((lines, score)) = lrclib_search_strict_scored(query, meta) {
            upsert_lrclib_best(&mut best, score, lines);
        }
    }

    if let Some((score, lines)) = best {
        if score >= LRCLIB_MIN_SCORE {
            return Ok((lines, score));
        }
    }
    Err("Letra sincronizada nao encontrada".into())
}

fn upsert_lrclib_best(best: &mut Option<(f64, Vec<LyricLine>)>, score: f64, lines: Vec<LyricLine>) {
    match best {
        None => *best = Some((score, lines)),
        Some((prev, _)) if score > *prev => *best = Some((score, lines)),
        _ => {}
    }
}

fn lrclib_get_validated_scored(
    track: &str,
    artist: &str,
    meta: &LyricsMeta,
) -> Result<(Vec<LyricLine>, f64), String> {
    let url = format!(
        "https://lrclib.net/api/get?track_name={}&artist_name={}",
        url_encode(track),
        url_encode(artist)
    );
    let body = http_get(&url)?;
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;

    if v.get("syncedLyrics").is_none() {
        return Err("LRCLIB get miss".into());
    }
    let score = lrclib_match_score(&v, meta);
    if score < LRCLIB_MIN_SCORE {
        return Err("LRCLIB get: faixa nao confere".into());
    }
    if let Some(lrc) = v.get("syncedLyrics").and_then(|x| x.as_str()) {
        if !lrc.trim().is_empty() {
            return Ok((parse_lrc(lrc)?, score));
        }
    }
    Err("LRCLIB get sem sync".into())
}

fn lrclib_search_strict_scored(query: &str, meta: &LyricsMeta) -> Result<(Vec<LyricLine>, f64), String> {
    let url = format!(
        "https://lrclib.net/api/search?q={}",
        url_encode(query.trim())
    );
    let body = http_get(&url)?;
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&body).map_err(|e| format!("LRCLIB search invalido: {e}"))?;

    let mut best: Option<(f64, String)> = None;
    for item in results {
        let score = lrclib_match_score(&item, meta);
        if score < LRCLIB_MIN_SCORE {
            continue;
        }
        let Some(lrc) = item.get("syncedLyrics").and_then(|x| x.as_str()) else {
            continue;
        };
        if lrc.trim().is_empty() {
            continue;
        }
        match &best {
            None => best = Some((score, lrc.to_string())),
            Some((prev, _)) if score > *prev => best = Some((score, lrc.to_string())),
            _ => {}
        }
    }

    if let Some((score, lrc)) = best {
        return Ok((parse_lrc(&lrc)?, score));
    }
    Err("LRCLIB search sem match".into())
}

fn lrclib_item_matches(item: &serde_json::Value, meta: &LyricsMeta) -> bool {
    lrclib_match_score(item, meta) >= 0.60
}

fn lrclib_match_score(item: &serde_json::Value, meta: &LyricsMeta) -> f64 {
    let item_artist = item
        .get("artistName")
        .and_then(|x| x.as_str())
        .unwrap_or("");
    let item_track = item
        .get("trackName")
        .and_then(|x| x.as_str())
        .unwrap_or("");
    if item_track.is_empty() {
        return 0.0;
    }

    let track_score = track_match_score(&meta.track_short, item_track)
        .max(track_match_score(&meta.track_full, item_track));
    if track_score < 0.5 {
        return 0.0;
    }

    if meta.artist.is_empty() {
        return if track_score >= 0.85 {
            track_score
        } else {
            0.0
        };
    }
    if !artists_match(item_artist, &meta.artist) {
        return 0.0;
    }
    (track_score + 1.0) / 2.0
}

fn track_match_score(expected: &str, found: &str) -> f64 {
    let e = normalize_track(expected);
    let f = normalize_track(found);
    if e.is_empty() || f.is_empty() {
        return 0.0;
    }
    if e == f {
        return 1.0;
    }
    if e.len() >= 5 && f.len() >= 5 && (e.contains(&f) || f.contains(&e)) {
        return 0.85;
    }
    word_jaccard(&e, &f)
}

fn normalize_track(s: &str) -> String {
    s.to_lowercase()
        .replace('&', "e")
        .replace('(', " ")
        .replace(')', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn word_jaccard(a: &str, b: &str) -> f64 {
    const STOP: &[&str] = &[
        "de", "da", "do", "e", "o", "a", "os", "as", "em", "na", "no", "um", "uma", "the", "of",
    ];
    let words_a: HashSet<String> = a
        .split_whitespace()
        .filter(|w| w.len() > 2 && !STOP.contains(w))
        .map(|w| w.to_string())
        .collect();
    let words_b: HashSet<String> = b
        .split_whitespace()
        .filter(|w| w.len() > 2 && !STOP.contains(w))
        .map(|w| w.to_string())
        .collect();
    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }
    let inter = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    inter as f64 / union as f64
}

fn artists_match(a: &str, b: &str) -> bool {
    let a = normalize_artist(a);
    let b = normalize_artist(b);
    a == b || a.contains(&b) || b.contains(&a)
}

fn normalize_artist(s: &str) -> String {
    s.to_lowercase()
        .replace("&", "e")
        .replace("  ", " ")
        .trim()
        .to_string()
}

fn fetch_youtube_subs(cookies: &str, video_id: &str) -> Result<Vec<LyricLine>, String> {
    if let Ok(lines) = download_subs_to_dir(cookies, video_id, false) {
        return Ok(lines);
    }
    if let Ok(lines) = download_subs_to_dir(cookies, video_id, true) {
        return Ok(lines);
    }
    download_subs_from_info(cookies, video_id)
}

fn download_subs_to_dir(cookies: &str, video_id: &str, auto: bool) -> Result<Vec<LyricLine>, String> {
    let ytdlp = find_ytdlp().ok_or("yt-dlp nao encontrado")?;
    let tmp = std::env::temp_dir().join(format!("promptub-lyrics-{video_id}"));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;

    let out_tpl = tmp.join("%(id)s").to_string_lossy().into_owned();
    let url = format!("https://www.youtube.com/watch?v={video_id}");

    for fmt in ["json3", "vtt", "srt"] {
        let mut args = vec![
            "--skip-download".into(),
            "--sub-langs".into(),
            "pt,pt-BR,en".into(),
            "--sub-format".into(),
            fmt.into(),
            "--extractor-args".into(),
            "youtube:player_client=android,web".into(),
            "--output".into(),
            out_tpl.clone(),
            url.clone(),
        ];
        if auto {
            args.insert(1, "--write-auto-subs".into());
        } else {
            args.insert(1, "--write-subs".into());
        };
        if !cookies.is_empty() {
            args.push("--cookies".into());
            args.push(cookies.into());
        }

        let _ = utf8_cmd(&ytdlp).args(&args).output();

        if let Ok(lines) = read_subs_from_dir(&tmp) {
            let _ = fs::remove_dir_all(&tmp);
            return Ok(lines);
        }
    }

    let _ = fs::remove_dir_all(&tmp);
    Err("Legendas do YouTube indisponiveis".into())
}

fn download_subs_from_info(cookies: &str, video_id: &str) -> Result<Vec<LyricLine>, String> {
    let info = ytdlp_dump_json(cookies, video_id)?;
    let sub_url = pick_sub_url(&info).ok_or("Sem URL de legenda")?;
    let body = http_get(&sub_url)?;
    parse_subtitle_text(&body, sub_url.contains("json3") || body.trim_start().starts_with('{'))
}

fn ytdlp_dump_json(cookies: &str, video_id: &str) -> Result<serde_json::Value, String> {
    let ytdlp = find_ytdlp().ok_or("yt-dlp nao encontrado")?;
    let url = format!("https://www.youtube.com/watch?v={video_id}");
    let mut args = vec![
        "--skip-download".into(),
        "--dump-single-json".into(),
        "--no-warnings".into(),
        "--extractor-args".into(),
        "youtube:player_client=android,web".into(),
        url,
    ];
    if !cookies.is_empty() {
        args.push("--cookies".into());
        args.push(cookies.into());
    }

    let output = utf8_cmd(&ytdlp).args(&args).output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(decode_bytes(&output.stderr));
    }
    serde_json::from_str(&decode_bytes(&output.stdout)).map_err(|e| e.to_string())
}

fn pick_sub_url(info: &serde_json::Value) -> Option<String> {
    if let Some(url) = pick_sub_url_from_key(info, "subtitles") {
        return Some(url);
    }
    pick_sub_url_from_key(info, "automatic_captions")
}

fn pick_sub_url_from_key(info: &serde_json::Value, key: &str) -> Option<String> {
    const LANGS: &[&str] = &["pt", "pt-BR", "pt-PT", "en", "en-US", "en-GB"];
    const FORMATS: &[&str] = &["json3", "srv3", "vtt", "srt"];

    let obj = info.get(key)?.as_object()?;

    for lang in LANGS {
        if let Some(url) = sub_url_for_lang(obj, lang, FORMATS) {
            return Some(url);
        }
    }

    for (lang, entries) in obj {
        if lang.starts_with("pt") || lang.starts_with("en") {
            if let Some(url) = sub_url_from_entries(entries, FORMATS) {
                return Some(url);
            }
        }
    }
    None
}

fn sub_url_for_lang(
    obj: &serde_json::Map<String, serde_json::Value>,
    lang: &str,
    formats: &[&str],
) -> Option<String> {
    obj.get(lang)
        .and_then(|entries| sub_url_from_entries(entries, formats))
}

fn sub_url_from_entries(entries: &serde_json::Value, formats: &[&str]) -> Option<String> {
    let arr = entries.as_array()?;
    for fmt in formats {
        for entry in arr {
            if entry.get("ext").and_then(|e| e.as_str()) == Some(*fmt) {
                if let Some(url) = entry.get("url").and_then(|u| u.as_str()) {
                    return Some(url.to_string());
                }
            }
        }
    }
    arr.first()
        .and_then(|e| e.get("url").and_then(|u| u.as_str()))
        .map(str::to_string)
}

fn read_subs_from_dir(dir: &Path) -> Result<Vec<LyricLine>, String> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_file() {
            files.push(path);
        }
    }

    files.sort_by_key(|p| sub_lang_priority(p));

    for path in files {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if let Ok(lines) = match ext.as_str() {
            "json3" => parse_json3(&raw),
            "vtt" => parse_vtt(&raw),
            "srt" => parse_srt(&raw),
            _ => parse_subtitle_text(&raw, raw.trim_start().starts_with('{')),
        } {
            if !lines.is_empty() {
                return Ok(lines);
            }
        }
    }
    Err("Nenhum arquivo de legenda legivel".into())
}

fn sub_lang_priority(path: &Path) -> u8 {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    if name.contains(".pt") {
        0
    } else if name.contains(".en") {
        1
    } else {
        2
    }
}

fn parse_subtitle_text(raw: &str, is_json: bool) -> Result<Vec<LyricLine>, String> {
    if is_json || raw.trim_start().starts_with('{') {
        return parse_json3(raw);
    }
    if raw.contains("-->") && raw.contains("WEBVTT") {
        return parse_vtt(raw);
    }
    if raw.contains("-->") {
        return parse_srt(raw);
    }
    parse_vtt(raw)
}

fn parse_json3(raw: &str) -> Result<Vec<LyricLine>, String> {
    let v: serde_json::Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let events = v
        .get("events")
        .and_then(|e| e.as_array())
        .ok_or("Formato json3 invalido")?;

    let mut lines = Vec::new();
    for ev in events {
        let start_ms = ev.get("tStartMs").and_then(|x| x.as_u64()).unwrap_or(0);
        let dur_ms = ev.get("dDurationMs").and_then(|x| x.as_u64()).unwrap_or(0);
        let text: String = ev
            .get("segs")
            .and_then(|s| s.as_array())
            .map(|segs| {
                segs.iter()
                    .filter_map(|seg| seg.get("utf8").and_then(|u| u.as_str()))
                    .collect::<String>()
            })
            .unwrap_or_default()
            .replace('\n', " ")
            .trim()
            .to_string();

        if text.is_empty() || text.chars().all(|c| matches!(c, '♪' | '♫' | ' ')) {
            continue;
        }
        if is_junk_lyric_line(&text) {
            continue;
        }

        let start = start_ms as f64 / 1000.0;
        let end = ((start_ms + dur_ms.max(500)) as f64) / 1000.0;
        lines.push(LyricLine { start, end, text, synced: true, source: default_source() });
    }

    finalize_line_ends(&mut lines);
    if lines.is_empty() {
        return Err("Legenda vazia".into());
    }
    Ok(lines)
}

fn parse_vtt(raw: &str) -> Result<Vec<LyricLine>, String> {
    let mut lines = Vec::new();
    let mut i = 0;
    let parts: Vec<&str> = raw.lines().collect();
    while i < parts.len() {
        let line = parts[i].trim();
        if line.contains("-->") {
            let (start, end) = parse_time_range(line);
            if let (Some(start), Some(end)) = (start, end) {
                let mut text = String::new();
                i += 1;
                while i < parts.len() {
                    let t = parts[i].trim();
                    if t.is_empty() || t.contains("-->") {
                        break;
                    }
                    if !t.starts_with("NOTE") && !t.chars().all(|c| c.is_ascii_digit() || c == ':') {
                        if !text.is_empty() {
                            text.push(' ');
                        }
                        text.push_str(t);
                    }
                    i += 1;
                }
                let text = clean_caption_text(&text);
                if !text.is_empty() && !is_junk_lyric_line(&text) {
                    lines.push(LyricLine { start, end, text, synced: true, source: default_source() });
                }
                continue;
            }
        }
        i += 1;
    }
    finalize_line_ends(&mut lines);
    if lines.is_empty() {
        return Err("VTT vazio".into());
    }
    Ok(lines)
}

fn parse_srt(raw: &str) -> Result<Vec<LyricLine>, String> {
    let mut lines = Vec::new();
    for block in raw.split("\n\n") {
        let mut block_lines = block.lines().filter(|l| !l.trim().is_empty());
        block_lines.next();
        let timing = block_lines.next().unwrap_or("");
        let (Some(start), Some(end)) = parse_time_range(timing) else {
            continue;
        };
        let text: String = block_lines
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let text = clean_caption_text(&text);
        if !text.is_empty() && !is_junk_lyric_line(&text) {
            lines.push(LyricLine { start, end, text, synced: true, source: default_source() });
        }
    }
    finalize_line_ends(&mut lines);
    if lines.is_empty() {
        return Err("SRT vazio".into());
    }
    Ok(lines)
}

fn parse_lrc(raw: &str) -> Result<Vec<LyricLine>, String> {
    let mut lines = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if !line.starts_with('[') {
            continue;
        }
        let mut rest = line;
        while rest.starts_with('[') {
            let end = rest.find(']').unwrap_or(0);
            if end == 0 {
                break;
            }
            let ts = &rest[1..end];
            if let Some(start) = parse_lrc_timestamp(ts) {
                rest = rest[end + 1..].trim();
                let text = clean_caption_text(rest);
                if !text.is_empty() && !rest.starts_with('[') && !is_junk_lyric_line(&text) {
                    lines.push(LyricLine {
                        start,
                        end: start + 4.0,
                        text,
                        synced: true,
                        source: default_source(),
                    });
                    break;
                }
            } else {
                break;
            }
        }
    }
    lines.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));
    finalize_line_ends(&mut lines);
    if lines.is_empty() {
        return Err("LRC vazio".into());
    }
    Ok(lines)
}

fn parse_time_range(line: &str) -> (Option<f64>, Option<f64>) {
    let parts: Vec<&str> = line.split("-->").collect();
    if parts.len() != 2 {
        return (None, None);
    }
    (
        parse_timestamp(parts[0].trim()),
        parse_timestamp(parts[1].split_whitespace().next().unwrap_or("").trim()),
    )
}

fn parse_timestamp(raw: &str) -> Option<f64> {
    let raw = raw.replace(',', ".");
    let pieces: Vec<&str> = raw.split(':').collect();
    match pieces.len() {
        3 => {
            let h: f64 = pieces[0].parse().ok()?;
            let m: f64 = pieces[1].parse().ok()?;
            let s: f64 = pieces[2].parse().ok()?;
            Some(h * 3600.0 + m * 60.0 + s)
        }
        2 => {
            let m: f64 = pieces[0].parse().ok()?;
            let s: f64 = pieces[1].parse().ok()?;
            Some(m * 60.0 + s)
        }
        _ => None,
    }
}

fn parse_lrc_timestamp(raw: &str) -> Option<f64> {
    parse_timestamp(raw)
}

fn finalize_line_ends(lines: &mut [LyricLine]) {
    for i in 0..lines.len() {
        if i + 1 < lines.len() {
            lines[i].end = lines[i + 1].start.max(lines[i].start + 0.5);
        } else if lines[i].end <= lines[i].start {
            lines[i].end = lines[i].start + 4.0;
        }
    }
}

fn clean_caption_text(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("<i>", "")
        .replace("</i>", "")
        .replace("<b>", "")
        .replace("</b>", "")
        .trim()
        .to_string()
}

fn is_junk_lyric_line(text: &str) -> bool {
    let t = text.trim();
    if t.len() < 2 {
        return true;
    }
    if t.len() > 160 {
        return true;
    }
    let lower = t.to_lowercase();
    if lower.contains("http")
        || lower.contains("www.")
        || lower.contains(".com")
        || lower.contains("google")
        || lower.contains("pesquis")
        || lower.contains("search")
        || lower.contains("bing")
        || lower.contains("yahoo")
    {
        return true;
    }
    const JUNK: &[&str] = &[
        "inscreva",
        "subscribe",
        "like and",
        "se inscrev",
        "ative o sininho",
        "clique em",
        "click ",
        "copyright",
        "legendas pela",
        "captions by",
        "amara.org",
        "sync by",
        "[music",
        "[música",
        "[musica",
        "[applause",
        "[risad",
        "whatsapp",
        "instagram",
        "facebook",
        "tiktok",
        "spotify",
        "deezer",
        "baixe ",
        "download",
        "visite ",
        "acesse ",
        "canal oficial",
        "official video",
        "video oficial",
        "produzido por",
        "all rights",
        "todos os direitos",
    ];
    for needle in JUNK {
        if lower.contains(needle) {
            return true;
        }
    }
    if t.chars().filter(|c| c.is_ascii_digit()).count() > t.len() / 3 {
        return true;
    }
    let letters = t.chars().filter(|c| c.is_alphabetic()).count();
    letters < t.len() / 4
}

fn sanitize_lyrics(mut lines: Vec<LyricLine>) -> Result<Vec<LyricLine>, String> {
    lines.retain(|l| !is_junk_lyric_line(&l.text));
    if lines.len() < 4 {
        return Err("Letra insuficiente apos filtro".into());
    }
    let substantive = lines
        .iter()
        .filter(|l| {
            let t = l.text.trim();
            t.len() >= 3 && t.chars().filter(|c| c.is_alphabetic()).count() >= 2
        })
        .count();
    if substantive < 4 || substantive * 100 / lines.len() < 60 {
        return Err("Legenda sem conteudo cantavel".into());
    }
    finalize_line_ends(&mut lines);
    Ok(lines)
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn http_get(url: &str) -> Result<String, String> {
    ureq::get(url)
        .set("User-Agent", "promptub/1.0 (+https://github.com/Joao-Savi/promptub)")
        .set("Accept", "application/json, text/plain, */*")
        .call()
        .map_err(|e| format!("HTTP: {e}"))?
        .into_string()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fetch_lyrics(
    state: tauri::State<'_, crate::state::SharedState>,
    video_id: String,
    title: Option<String>,
    artist: Option<String>,
) -> Result<Vec<LyricLine>, String> {
    let id = video_id.trim().to_string();
    let title = title.unwrap_or_default();
    let artist = artist.unwrap_or_default();
    let cookies = state.cookies();
    tauri::async_runtime::spawn_blocking(move || lookup_lyrics(&cookies, &id, &title, &artist))
        .await
        .map_err(|e| format!("lyrics: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_jorge_mateus_title() {
        let meta = parse_lyrics_meta(
            "Jorge & Mateus - Paredes (Como Sempre Feito Nunca) [Video Oficial]",
            "Som Livre",
        );
        assert_eq!(meta.artist, "Jorge & Mateus");
        assert_eq!(meta.track_short, "Paredes");
        assert!(meta.track_full.contains("Paredes"));
    }

    #[test]
    fn parses_lrc_timestamps() {
        let lrc = "[00:06.05] Despertador tocou\n[00:08.50] Pra me dar o beijo";
        let lines = parse_lrc(lrc).unwrap();
        assert_eq!(lines.len(), 2);
        assert!((lines[0].start - 6.05).abs() < 0.01);
        assert!(lines[0].text.contains("Despertador"));
    }

    #[test]
    fn rejects_wrong_lrclib_track() {
        let meta = parse_lyrics_meta(
            "Gusttavo Lima - Que Mal Te Fiz Eu (Diz-Me)",
            "Gusttavo Lima",
        );
        let wrong = serde_json::json!({
            "artistName": "Gusttavo Lima",
            "trackName": "Deixa Ser",
            "syncedLyrics": "[00:01.00] E a guitarra soa"
        });
        assert!(!lrclib_item_matches(&wrong, &meta));
    }

    #[test]
    fn accepts_matching_lrclib_track() {
        let meta = parse_lyrics_meta(
            "Gusttavo Lima - Que Mal Te Fiz Eu (Diz-Me)",
            "Gusttavo Lima",
        );
        let ok = serde_json::json!({
            "artistName": "Gusttavo Lima",
            "trackName": "Que Mal Te Fiz Eu",
            "syncedLyrics": "[00:01.00] test"
        });
        assert!(lrclib_item_matches(&ok, &meta));
    }

    #[test]
    fn rejects_loose_track_without_artist() {
        let meta = parse_lyrics_meta("Deixa Ser", "");
        let item = serde_json::json!({
            "artistName": "Outro",
            "trackName": "Que Mal Te Fiz Eu",
            "syncedLyrics": "[00:01.00] test"
        });
        assert!(!lrclib_item_matches(&item, &meta));
    }

    #[test]
    fn accepts_exact_track_without_artist() {
        let meta = parse_lyrics_meta("Deixa Ser", "");
        let item = serde_json::json!({
            "artistName": "Outro",
            "trackName": "Deixa Ser",
            "syncedLyrics": "[00:01.00] test"
        });
        assert!(lrclib_item_matches(&item, &meta));
    }

    #[test]
    fn prefers_youtube_on_borderline_lrclib() {
        let lrc = vec![LyricLine {
            start: 1.0,
            end: 2.0,
            text: "test".into(),
            synced: true,
            source: "lrclib".into(),
        }];
        let yt = vec![LyricLine {
            start: 1.0,
            end: 2.0,
            text: "video".into(),
            synced: true,
            source: "youtube".into(),
        }];
        let picked = pick_lyrics_result(Some((lrc.clone(), 0.7)), Some(yt.clone())).unwrap();
        assert_eq!(picked[0].text, "video");
        let picked_strong = pick_lyrics_result(Some((lrc, 0.8)), Some(yt)).unwrap();
        assert_eq!(picked_strong[0].text, "test");
    }
}
