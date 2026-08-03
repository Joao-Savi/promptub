use crate::queue::Queue;
use crate::history::WatchHistory;
use crate::stream::StreamCache;
use crate::youtube::Video;
use parking_lot::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::AppHandle;

pub struct AppState {
    pub queue: Mutex<Queue>,
    pub cookies_path: Mutex<String>,
    pub last_search: Mutex<String>,
    pub last_video: Mutex<Option<Video>>,
    pub stream_cache: StreamCache,
    pub refill_in_progress: AtomicBool,
    pub refill_generation: std::sync::atomic::AtomicUsize,
    pub watch_history: Mutex<WatchHistory>,
    pub app_handle: Mutex<Option<AppHandle>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(Queue::new()),
            cookies_path: Mutex::new(String::new()),
            last_search: Mutex::new(String::new()),
            last_video: Mutex::new(None),
            stream_cache: StreamCache::new(),
            refill_in_progress: AtomicBool::new(false),
            refill_generation: std::sync::atomic::AtomicUsize::new(0),
            watch_history: Mutex::new(WatchHistory::load()),
            app_handle: Mutex::new(None),
        }
    }

    pub fn set_app_handle(&self, handle: AppHandle) {
        *self.app_handle.lock() = Some(handle);
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

    pub fn set_last_video(&self, video: Video) {
        *self.last_video.lock() = Some(video);
    }

    pub fn last_video_id(&self) -> Option<String> {
        self.last_video
            .lock()
            .as_ref()
            .map(|v| v.id.clone())
    }
}

pub type SharedState = Arc<AppState>;
