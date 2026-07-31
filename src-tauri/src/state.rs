use crate::player::Player;
use crate::queue::Queue;
use crate::stream::StreamCache;
use crate::youtube::Video;
use parking_lot::Mutex;
use std::sync::Arc;

pub struct AppState {
    pub queue: Mutex<Queue>,
    pub player: Mutex<Player>,
    pub cookies_path: Mutex<String>,
    pub audio_only: Mutex<bool>,
    pub last_search: Mutex<String>,
    pub last_video: Mutex<Option<Video>>,
    pub last_music_video: Mutex<Option<Video>>,
    pub last_watch_video: Mutex<Option<Video>>,
    pub stream_cache: StreamCache,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(Queue::new()),
            player: Mutex::new(Player::new()),
            cookies_path: Mutex::new(String::new()),
            audio_only: Mutex::new(false),
            last_search: Mutex::new(String::new()),
            last_video: Mutex::new(None),
            last_music_video: Mutex::new(None),
            last_watch_video: Mutex::new(None),
            stream_cache: StreamCache::new(),
        }
    }

    pub fn cookies(&self) -> String {
        self.cookies_path.lock().clone()
    }

    pub fn set_cookies(&self, path: String) {
        *self.cookies_path.lock() = path;
    }

    pub fn set_last_search(&self, query: String) {
        *self.last_search.lock() = query;
    }

    pub fn last_search(&self) -> String {
        self.last_search.lock().clone()
    }

    pub fn set_last_video(&self, video: Video, audio_only: bool) {
        *self.last_video.lock() = Some(video.clone());
        if audio_only {
            *self.last_music_video.lock() = Some(video);
        } else {
            *self.last_watch_video.lock() = Some(video);
        }
    }

    pub fn last_video_id(&self) -> Option<String> {
        self.last_video
            .lock()
            .as_ref()
            .map(|v| v.id.clone())
    }
}

pub type SharedState = Arc<AppState>;
