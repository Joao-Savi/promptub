//! Limpeza de caches antigos — disco (%APPDATA%\promptub) e temp.

use crate::stream::StreamCache;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const TEMP_MAX_AGE: Duration = Duration::from_secs(60 * 60);
const APPDATA_ORPHAN_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);

pub fn run_startup_cleanup(stream_cache: &StreamCache) {
    stream_cache.prune_expired();
    cleanup_temp_promptub_dirs();
    cleanup_stale_appdata_orphans();
}

pub fn schedule_periodic_cleanup(stream_cache: StreamCache) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(30 * 60));
        stream_cache.prune_expired();
        cleanup_temp_promptub_dirs();
    });
}

fn promptub_appdata_dir() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("promptub")
}

fn cleanup_temp_promptub_dirs() {
    let tmp = std::env::temp_dir();
    let Ok(read) = fs::read_dir(&tmp) else {
        return;
    };
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("promptub-lyrics-") && !name.starts_with("promptub-export-") {
            continue;
        }
        if path_older_than(&entry.path(), TEMP_MAX_AGE) {
            let _ = fs::remove_dir_all(entry.path()).or_else(|_| fs::remove_file(entry.path()));
        }
    }
}

/// Remove backups/orfaos muito antigos (nao mexe em history/feed ativos).
fn cleanup_stale_appdata_orphans() {
    let dir = promptub_appdata_dir();
    let Ok(read) = fs::read_dir(&dir) else {
        return;
    };
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        let stale = name.ends_with(".bak")
            || name.ends_with(".old")
            || name.starts_with("stream_cache.")
            || name.starts_with("feed_cache.");
        if stale && path_older_than(&path, APPDATA_ORPHAN_MAX_AGE) {
            let _ = fs::remove_file(&path);
        }
    }
}

fn path_older_than(path: &Path, max_age: Duration) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return true;
    };
    modified
        .elapsed()
        .map(|e| e > max_age)
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_prefixes_match() {
        assert!("promptub-lyrics-abc".starts_with("promptub-lyrics-"));
        assert!("promptub-export-1.txt".starts_with("promptub-export-"));
    }
}
