use crate::deps::{find_ytdlp, utf8_cmd};
use crate::discover::extract_artist_label;
use crate::text::decode_bytes;
use crate::youtube::Video;
use serde::Serialize;
use std::fs;
use std::path::Path;

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

    // LRCLIB primeiro (~1 s); legendas YouTube so se faltar (yt-dlp e lento e adianta)
    if let Ok(lines) = fetch_lrclib(&meta, true) {
        if let Ok(clean) = sanitize_lyrics(lines) {
            return Ok(tag_source(clean, "lrclib"));
        }
    }

    if let Ok(lines) = fetch_lrclib(&meta, false) {
        if let Ok(clean) = sanitize_lyrics(lines) {
            let src = if clean.iter().all(|l| l.synced) {
                "lrclib"
            } else {
                "plain"
            };
            return Ok(tag_source(clean, src));
        }
    }

    if let Ok(lines) = fetch_youtube_subs(cookies, id) {
        if let Ok(clean) = sanitize_lyrics(lines) {
            return Ok(tag_source(clean, "youtube"));
        }
    }

    Err("Letra nao encontrada para esta faixa".into())
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

fn fetch_lrclib(meta: &LyricsMeta, synced_only: bool) -> Result<Vec<LyricLine>, String> {
    const NO_ARTIST: &str = "";
    let pairs = [
        (&meta.track_short, meta.artist.as_str()),
        (&meta.track_full, meta.artist.as_str()),
        (&meta.track_short, NO_ARTIST),
        (&meta.track_full, NO_ARTIST),
    ];

    for (track, artist) in pairs {
        if track.is_empty() {
            continue;
        }
        if let Ok(lines) = lrclib_get(track, artist, synced_only) {
            return Ok(lines);
        }
    }

    for query in [
        &meta.search_query,
        &format!("{} {}", meta.artist, meta.track_short),
        &meta.track_short,
        &meta.track_full,
    ] {
        if query.trim().is_empty() {
            continue;
        }
        if let Ok(lines) = lrclib_search(query, &meta.artist, synced_only) {
            return Ok(lines);
        }
    }

    Err(if synced_only {
        "Letra sincronizada nao encontrada".into()
    } else {
        "Letra nao encontrada".into()
    })
}

fn lrclib_get(track: &str, artist: &str, synced_only: bool) -> Result<Vec<LyricLine>, String> {
    let url = if artist.trim().is_empty() {
        format!(
            "https://lrclib.net/api/get?track_name={}",
            url_encode(track)
        )
    } else {
        format!(
            "https://lrclib.net/api/get?track_name={}&artist_name={}",
            url_encode(track),
            url_encode(artist)
        )
    };
    let body = http_get(&url)?;
    parse_lrclib_response(&body, artist, synced_only)
}

fn lrclib_search(query: &str, prefer_artist: &str, synced_only: bool) -> Result<Vec<LyricLine>, String> {
    let url = format!(
        "https://lrclib.net/api/search?q={}",
        url_encode(query.trim())
    );
    let body = http_get(&url)?;
    let raw = body;
    let results: Vec<serde_json::Value> =
        serde_json::from_str(&raw).map_err(|e| format!("LRCLIB search invalido: {e}"))?;

    let mut fallback: Option<String> = None;
    let mut plain_fallback: Option<String> = None;
    for item in results {
        if let Some(lrc) = item.get("syncedLyrics").and_then(|x| x.as_str()) {
            if !lrc.trim().is_empty() {
                let item_artist = item
                    .get("artistName")
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                if !prefer_artist.is_empty() && artists_match(item_artist, prefer_artist) {
                    return parse_lrc(lrc);
                }
                if fallback.is_none() {
                    fallback = Some(lrc.to_string());
                }
            }
        }
        if plain_fallback.is_none() {
            if let Some(plain) = item.get("plainLyrics").and_then(|x| x.as_str()) {
                if !plain.trim().is_empty() {
                    plain_fallback = Some(plain.to_string());
                }
            }
        }
    }

    if let Some(lrc) = fallback {
        return parse_lrc(&lrc);
    }
    if !synced_only {
        if let Some(plain) = plain_fallback {
            return Ok(plain_to_lines(&plain));
        }
    }
    Err("LRCLIB sem letra".into())
}

fn parse_lrclib_response(raw: &str, prefer_artist: &str, synced_only: bool) -> Result<Vec<LyricLine>, String> {
    let v: serde_json::Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    if let Some(arr) = v.as_array() {
        return lrclib_search_result(arr, prefer_artist, synced_only);
    }
    if let Some(lrc) = v.get("syncedLyrics").and_then(|x| x.as_str()) {
        if !lrc.trim().is_empty() {
            return parse_lrc(lrc);
        }
    }
    if !synced_only {
        if let Some(plain) = v.get("plainLyrics").and_then(|x| x.as_str()) {
            if !plain.trim().is_empty() {
                return Ok(plain_to_lines(plain));
            }
        }
    }
    Err("LRCLIB resposta sem letra".into())
}

fn plain_to_lines(text: &str) -> Vec<LyricLine> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .enumerate()
        .map(|(i, line)| LyricLine {
            start: i as f64 * 3.0,
            end: (i + 1) as f64 * 3.0,
            text: line.to_string(),
            synced: false,
            source: default_source(),
        })
        .collect()
}

fn lrclib_search_result(items: &[serde_json::Value], prefer_artist: &str, synced_only: bool) -> Result<Vec<LyricLine>, String> {
    let mut fallback: Option<&str> = None;
    let mut plain_fallback: Option<&str> = None;
    for item in items {
        if let Some(lrc) = item.get("syncedLyrics").and_then(|x| x.as_str()) {
            if lrc.trim().is_empty() {
                continue;
            }
            let item_artist = item
                .get("artistName")
                .and_then(|x| x.as_str())
                .unwrap_or("");
            if !prefer_artist.is_empty() && artists_match(item_artist, prefer_artist) {
                return parse_lrc(lrc);
            }
            if fallback.is_none() {
                fallback = Some(lrc);
            }
        }
        if plain_fallback.is_none() {
            if let Some(plain) = item.get("plainLyrics").and_then(|x| x.as_str()) {
                if !plain.trim().is_empty() {
                    plain_fallback = Some(plain);
                }
            }
        }
    }
    if let Some(lrc) = fallback {
        return parse_lrc(lrc);
    }
    if !synced_only {
        if let Some(plain) = plain_fallback {
            return Ok(plain_to_lines(plain));
        }
    }
    Err("LRCLIB array vazio".into())
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
}
