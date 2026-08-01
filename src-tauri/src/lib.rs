mod auth;
mod deps;
mod discover;
mod history;
mod ipc;
mod player;
mod queue;
mod queue_refill;
mod recommend;
mod state;
mod stream;
mod text;
mod video_discover;
mod video_recommend;
mod video_embed;
mod youtube;

use state::{AppState, SharedState};
use std::sync::Arc;
use std::time::Duration;
use tauri::{Manager, RunEvent, WindowEvent};

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
fn boot_mode() -> String {
    if std::env::args().any(|a| a == "--screenshot-video") {
        "video".into()
    } else {
        "music".into()
    }
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
                let watch_state = state.clone();
                std::thread::spawn(move || player::watch_end_events(watch_state));

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
        .on_window_event({
            let state = state.clone();
            move |window, event| {
                if window.label() != "main" {
                    return;
                }
                match event {
                    WindowEvent::CloseRequested { .. } => {
                        player::shutdown_player(&state);
                    }
                    WindowEvent::Focused(focused) => {
                        if !focused {
                            crate::video_embed::set_host_visible(false);
                        }
                    }
                    _ => {}
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            boot_mode,
            app_version,
            deps::check_deps,
            youtube::search,
            youtube::resolve_video,
            youtube::related,
            youtube::video_context_feed,
            youtube::home_recommendations,
            recommend::recommended_playlist,
            player::warmup,
            player::resolve_stream,
            player::prewarm_streams,
            player::play,
            player::stop,
            player::next,
            player::prev,
            player::get_volume,
            player::set_volume,
            player::sync_video_panel,
            player::set_video_overlay_visible,
            player::hide_video_panel,
            player::get_video_quality,
            player::set_video_quality,
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
            auth::has_premium_session,
        ])
        .build(tauri::generate_context!())
        .expect("error building tauri");

    let state_exit = state.clone();
    app.run(move |_app, event| {
        if let RunEvent::Exit = event {
            player::shutdown_player(&state_exit);
        }
    });
}
