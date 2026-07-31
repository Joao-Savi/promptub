mod auth;
mod deps;
mod ipc;
mod player;
mod queue;
mod recommend;
mod state;
mod stream;
mod text;
mod video_embed;
mod youtube;

use state::{AppState, SharedState};
use std::sync::Arc;
use std::time::Duration;
use tauri::{Manager, RunEvent, WindowEvent};

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state: SharedState = Arc::new(AppState::new());

    let app = tauri::Builder::default()
        .manage(state.clone())
        .setup({
            let state = state.clone();
            move |app| {
                init_tools(app.handle());
                auth::load_cookies(&state);
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
                if let WindowEvent::CloseRequested { .. } = event {
                    if window.label() == "main" {
                        player::shutdown_player(&state);
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            boot_mode,
            deps::check_deps,
            youtube::search,
            youtube::related,
            youtube::home_recommendations,
            recommend::recommended_playlist,
            player::warmup,
            player::play,
            player::stop,
            player::next,
            player::prev,
            player::get_volume,
            player::set_volume,
            player::sync_video_panel,
            player::hide_video_panel,
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
