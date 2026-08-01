//! Busca de faixas para recarga da fila com variedade (genero/estilo, nao so mesmo artista).

use crate::youtube::Video;

struct GenreProfile {
    triggers: &'static [&'static str],
    styles: &'static [&'static str],
}

const GENRE_PROFILES: &[GenreProfile] = &[
    GenreProfile {
        triggers: &["sertanejo", "modao", "modão", "sofrencia", "sofrência", "arrocha"],
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

/// Consultas focadas em genero/estilo a partir da busca e da faixa atual.
pub fn genre_search_queries(last_search: &str, seed: &Video, rotation: usize) -> Vec<String> {
    let mut out = Vec::new();
    let ls = last_search.trim();
    let context = if ls.is_empty() {
        simplify_for_search(&seed.title)
    } else {
        format!("{ls} {}", simplify_for_search(&seed.title))
    };

    let styles = expand_style_queries(&context, rotation);
    if !styles.is_empty() {
        out.extend(styles);
        if !ls.is_empty() {
            out.push(format!("{ls} mix"));
            out.push(format!("{ls} playlist"));
        }
    } else if !ls.is_empty() {
        out.push(format!("{ls} estilo"));
        out.push(format!("{ls} genero"));
        out.push(format!("{ls} similar"));
        out.push(format!("{ls} radio"));
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
        if let Some(first_profile) = matched_profiles(&context).first() {
            let accent = first_profile.styles[rotation % first_profile.styles.len()];
            out.push(format!("{theme_only} {accent}"));
        } else {
            out.push(theme_only.clone());
            out.push(format!("{theme_only} musica"));
        }
    }

    out.sort();
    out.dedup();
    out
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
    fn expands_sertanejo_subgenres() {
        let q = genre_search_queries("sertanejo romantico", &seed("Amor Covarde"), 0);
        assert!(q.iter().any(|s| s.contains("universitario")));
        assert!(q.iter().any(|s| s.contains("modao")));
    }

    #[test]
    fn rotates_styles() {
        let a = genre_search_queries("sertanejo", &seed("Teste"), 0);
        let b = genre_search_queries("sertanejo", &seed("Teste"), 3);
        assert_ne!(a.first(), b.first());
    }
}
