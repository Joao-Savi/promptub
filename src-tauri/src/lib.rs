mod lyrics;
mod auth;
mod deps;
mod discover;
mod feed_cache;
mod feed_sections;
mod history;
mod player;
mod queue;
mod queue_refill;
mod music_recommend;
mod recommend;
mod state;
mod stream;
mod taste;
mod text;
mod youtube;

use state::{AppState, SharedState};
use std::sync::Arc;
use std::time::Duration;
use tauri::{Manager, RunEvent};

fn run_test_search(query: &str) -> ! {
    deps::init_bundled_tools(None);
    let ytdlp = deps::find_ytdlp().unwrap_or_else(|| {
        eprintln!("yt-dlp nao encontrado");
        std::process::exit(1);
    });
    eprintln!("yt-dlp: {ytdlp}");
    match youtube::fetch_search("", query, youtube::SEARCH_LIMIT) {
        Ok(videos) => {
            println!("{}", serde_json::to_string(&videos).unwrap_or_else(|_| "[]".into()));
            std::process::exit(if videos.is_empty() { 2 } else { 0 });
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

fn init_tools(handle: &tauri::AppHandle) {
    let tools = handle
        .path()
        .resource_dir()
        .ok()
        .map(|d| d.join("tools"));
    deps::init_bundled_tools(tools);
}

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Some(q) = std::env::args().find_map(|a| {
        a.strip_prefix("--test-search=")
            .map(|s| s.to_string())
    }) {
        run_test_search(&q);
    }

    let state: SharedState = Arc::new(AppState::new());

    let app = tauri::Builder::default()
        .manage(state.clone())
        .setup({
            let state = state.clone();
            move |app| {
                init_tools(app.handle());
                state.set_app_handle(app.handle().clone());
                auth::load_cookies(&state);
                {
                    let history = state.watch_history.lock().clone();
                    history::hydrate_state(&state, &history);
                }

                if let Some(window) = app.get_webview_window("main") {
                    let win = window.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(400));
                        let _ = win.set_focus();
                    });
                }

                Ok(())
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_version,
            deps::check_deps,
            youtube::search,
            youtube::resolve_video,
            feed_cache::get_stored_feed,
            feed_cache::save_stored_feed,
            feed_sections::home_feed_local,
            feed_sections::home_feed_section,
            recommend::recommended_playlist,
            player::resolve_stream,
            player::play,
            player::next,
            player::prev,
            player::prewarm_playlist,
            player::prewarm_status,
            queue::enqueue,
            queue::get_queue,
            queue::clear_queue,
            queue::remove_queue_item,
            queue::load_queue,
            queue::play_queue_item,
            auth::is_logged_in,
            auth::login,
            auth::logout,
            lyrics::fetch_lyrics_cmd,
            taste::taste_like,
            taste::taste_dislike,
            taste::taste_get,
        ])
        .build(tauri::generate_context!())
        .expect("error building tauri");

    app.run(|_app, event| {
        if let RunEvent::Exit = event {}
    });
}
