use crate::deps::{find_mpv, mpv_cmd};
use crate::ipc;
use crate::state::SharedState;
use crate::youtube::Video;
use serde_json::json;
use std::io::{Read, Write};
use std::process::Child;
use std::thread;
use std::time::Duration;

const FORMAT_AUDIO: &str = "bestaudio[ext=m4a]/bestaudio/best";
const FORMAT_VIDEO: &str =
    "bestvideo[height<=720][ext=mp4]+bestaudio[ext=m4a]/bestvideo[height<=720]+bestaudio/b[height<=720]/best";

#[derive(Clone, Copy, PartialEq, Eq)]
enum MpvWindow {
    Audio,
    Video { x: i32, y: i32, w: i32, h: i32 },
}

pub struct Player {
    child: Option<Child>,
    cookies_path: String,
    pipe_name: String,
    mpv_window: MpvWindow,
}

impl Player {
    pub fn new() -> Self {
        Self {
            child: None,
            cookies_path: String::new(),
            pipe_name: format!("promptub-{}", std::process::id()),
            mpv_window: MpvWindow::Audio,
        }
    }

    fn pipe_path(&self) -> String {
        format!(r"\\.\pipe\{}", self.pipe_name)
    }

    pub fn set_cookies(&mut self, path: String) {
        if self.cookies_path != path {
            self.cookies_path = path;
            let _ = self.restart();
        }
    }

    pub fn set_video_area(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let next = MpvWindow::Video { x, y, w, h };
        if self.mpv_window == next {
            return;
        }
        self.mpv_window = next;
        if self.daemon_responding() {
            let _ = self.restart();
        }
    }

    pub fn clear_video_area(&mut self) {
        if self.mpv_window == MpvWindow::Audio {
            return;
        }
        self.mpv_window = MpvWindow::Audio;
        if self.daemon_responding() {
            let _ = self.restart();
        }
    }

    pub fn warmup(&mut self) -> Result<(), String> {
        self.ensure_daemon()
    }

    pub fn play(
        &mut self,
        video: &Video,
        audio_only: bool,
        direct_url: Option<&str>,
    ) -> Result<(), String> {
        if audio_only {
            if self.mpv_window != MpvWindow::Audio {
                self.mpv_window = MpvWindow::Audio;
                self.restart()?;
            }
        } else if !matches!(self.mpv_window, MpvWindow::Video { .. }) {
            return Err("painel de video nao sincronizado — tente novamente".into());
        }

        self.ensure_daemon()?;

        if audio_only {
            ipc_property(&self.pipe_name, "force-window", json!(false))?;
            ipc_property(&self.pipe_name, "video", json!(false))?;
        } else {
            ipc_property(&self.pipe_name, "force-window", json!(true))?;
            ipc_property(&self.pipe_name, "video", json!(true))?;
            if let MpvWindow::Video { x, y, w, h } = self.mpv_window {
                let _ = ipc_property(
                    &self.pipe_name,
                    "geometry",
                    json!(format!("{w}x{h}+{x}+{y}")),
                );
            }
        }

        let ytdl_raw = if audio_only {
            self.ytdl_raw_options("android")
        } else {
            self.ytdl_raw_options("android,web")
        };
        ipc_property(&self.pipe_name, "ytdl-raw-options", json!(ytdl_raw))?;

        if let Some(url) = direct_url.filter(|_| audio_only) {
            ipc_property(&self.pipe_name, "ytdl", json!(false))?;
            if ipc_loadfile(&self.pipe_name, url).is_err() {
                self.restart()?;
                ipc_property(&self.pipe_name, "ytdl", json!(false))?;
                ipc_loadfile(&self.pipe_name, url)?;
            }
            let _ = ipc_property(&self.pipe_name, "ytdl", json!(true));
        } else {
            let format = if audio_only { FORMAT_AUDIO } else { FORMAT_VIDEO };
            ipc_property(&self.pipe_name, "ytdl", json!(true))?;
            ipc_property(&self.pipe_name, "ytdl-format", json!(format))?;
            if ipc_loadfile(&self.pipe_name, &video.url).is_err() {
                if !audio_only {
                    for client in ["web", "ios", "mweb"] {
                        let fallback = self.ytdl_raw_options(client);
                        ipc_property(&self.pipe_name, "ytdl-raw-options", json!(fallback))?;
                        if ipc_loadfile(&self.pipe_name, &video.url).is_ok() {
                            return Ok(());
                        }
                    }
                }
                self.restart()?;
                ipc_property(&self.pipe_name, "ytdl", json!(true))?;
                ipc_property(&self.pipe_name, "ytdl-format", json!(format))?;
                ipc_property(&self.pipe_name, "ytdl-raw-options", json!(ytdl_raw))?;
                ipc_loadfile(&self.pipe_name, &video.url)?;
            }
        }
        Ok(())
    }

    fn ytdl_raw_options(&self, client: &str) -> String {
        let mut raw = format!(
            "quiet=,no-warnings=,no-progress=,extractor-args=youtube:player_client={client}"
        );
        if !self.cookies_path.is_empty() {
            let path = self.cookies_path.replace('\\', "/");
            raw.push_str(&format!(",cookies={path}"));
        }
        raw
    }

    pub fn stop(&mut self) -> Result<(), String> {
        ipc_cmd(&self.pipe_name, &["stop"])
    }

    pub fn volume(&mut self) -> Result<f64, String> {
        if !self.daemon_responding() {
            return Ok(100.0);
        }
        let v = ipc_get_property(&self.pipe_name, "volume")?;
        v.as_f64().ok_or_else(|| "volume invalido".into())
    }

    pub fn set_volume(&mut self, level: f64) -> Result<(), String> {
        self.ensure_daemon()?;
        ipc_property(
            &self.pipe_name,
            "volume",
            json!(level.clamp(0.0, 100.0)),
        )
    }

    fn daemon_responding(&mut self) -> bool {
        if let Some(child) = self.child.as_mut() {
            if child.try_wait().ok().flatten().is_some() {
                return false;
            }
        } else {
            return false;
        }
        ipc_cmd(&self.pipe_name, &["get_property", "idle-active"]).is_ok()
    }

    fn ensure_daemon(&mut self) -> Result<(), String> {
        if self.child.as_mut().is_some_and(|c| c.try_wait().ok().flatten().is_none()) {
            if ipc_cmd(&self.pipe_name, &["get_property", "idle-active"]).is_ok() {
                return Ok(());
            }
            self.shutdown();
        }
        self.start_daemon()
    }

    fn restart(&mut self) -> Result<(), String> {
        self.shutdown();
        thread::sleep(Duration::from_millis(200));
        self.start_daemon()
    }

    fn start_daemon(&mut self) -> Result<(), String> {
        let mpv = find_mpv().ok_or("mpv nao encontrado. Reinstale o promptub.")?;
        let ytdl_raw = self.ytdl_raw_options("android");

        let mut args = vec![
            "--idle=yes".to_string(),
            "--keep-open=yes".to_string(),
            format!("--input-ipc-server={}", self.pipe_path()),
            "--ytdl=yes".to_string(),
            format!("--ytdl-raw-options={ytdl_raw}"),
            "--no-terminal".to_string(),
            "--really-quiet".to_string(),
            "--cache=yes".to_string(),
            "--demuxer-readahead-secs=5".to_string(),
            "--prefetch-playlist=yes".to_string(),
        ];

        match self.mpv_window {
            MpvWindow::Audio => {
                args.push("--force-window=no".to_string());
            }
            MpvWindow::Video { x, y, w, h } => {
                args.push("--force-window=yes".to_string());
                args.push(format!("--geometry={w}x{h}+{x}+{y}"));
                args.push("--border=no".to_string());
                args.push("--ontop".to_string());
                args.push("--title=promptub stream".to_string());
            }
        }

        let child = mpv_cmd(&mpv)
            .args(&args)
            .spawn()
            .map_err(|e| format!("mpv start: {e}"))?;
        self.child = Some(child);

        for i in 0..100 {
            if self.child.as_mut().is_some_and(|c| c.try_wait().ok().flatten().is_some()) {
                return Err("mpv encerrou ao iniciar. Reinstale o mpv ou reinicie o app.".into());
            }
            if ipc_cmd(&self.pipe_name, &["get_property", "idle-active"]).is_ok() {
                let _ = ipc_property(&self.pipe_name, "focus-on-open", json!(false));
                return Ok(());
            }
            thread::sleep(Duration::from_millis(if i < 30 { 100 } else { 50 }));
        }
        self.shutdown();
        Err("mpv IPC timeout. Feche outros mpv abertos e tente de novo.".into())
    }

    pub fn shutdown(&mut self) {
        let _ = ipc_cmd(&self.pipe_name, &["quit"]);
        if let Some(mut child) = self.child.take() {
            kill_process_tree(&mut child);
        }
        thread::sleep(Duration::from_millis(100));
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub(crate) fn play_cached(state: &SharedState, video: &Video, audio_only: bool) -> Result<(), String> {
    let direct = if audio_only {
        state.stream_cache.get(&video.id)
    } else {
        None
    };
    state
        .player
        .lock()
        .play(video, audio_only, direct.as_deref())
}

#[cfg(windows)]
fn kill_process_tree(child: &mut Child) {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let pid = child.id();
    let _ = Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(not(windows))]
fn kill_process_tree(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

pub fn shutdown_player(state: &SharedState) {
    state.player.lock().shutdown();
}

pub fn watch_end_events(state: SharedState) {
    loop {
        let pipe = {
            let player = state.player.lock();
            player.pipe_name.clone()
        };

        let Ok(mut client) = ipc::dial(&pipe) else {
            thread::sleep(Duration::from_millis(500));
            continue;
        };

        let observe = r#"{"command":["enable_event","end-file",{"mode":"observe"}]}"#;
        let _ = client.write_all(observe.as_bytes());
        let _ = client.write_all(b"\n");

        let mut buf = [0u8; 8192];
        loop {
            match client.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buf[..n]);
                    for line in text.lines() {
                        if !line.contains("end-file") {
                            continue;
                        }
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                            let reason = v.get("reason").and_then(|r| r.as_i64()).unwrap_or(-1);
                            if reason != 0 {
                                continue;
                            }
                            let audio_only = *state.audio_only.lock();
                            if let Some(video) = state.queue.lock().next() {
                                let _ = play_cached(&state, &video, audio_only);
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }
        thread::sleep(Duration::from_millis(300));
    }
}

fn ipc_property(pipe: &str, name: &str, value: serde_json::Value) -> Result<(), String> {
    send_ipc(pipe, &json!({ "command": ["set_property", name, value] }))
}

fn ipc_get_property(pipe: &str, name: &str) -> Result<serde_json::Value, String> {
    let resp = ipc::send(
        &json!({ "command": ["get_property", name] }).to_string(),
        pipe,
    )?;
    let v: serde_json::Value =
        serde_json::from_str(resp.lines().next().unwrap_or("")).map_err(|e| e.to_string())?;
    if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
        if err != "success" {
            return Err(format!("mpv: {err}"));
        }
    }
    v.get("data")
        .cloned()
        .ok_or_else(|| "mpv: sem dados".into())
}

fn ipc_cmd(pipe: &str, args: &[&str]) -> Result<(), String> {
    let arr: Vec<serde_json::Value> = args.iter().map(|s| json!(s)).collect();
    send_ipc(pipe, &json!({ "command": arr }))
}

fn ipc_loadfile(pipe: &str, url: &str) -> Result<(), String> {
    send_ipc(pipe, &json!({ "command": ["loadfile", url, "replace"] }))
}

fn send_ipc(pipe: &str, payload: &serde_json::Value) -> Result<(), String> {
    let resp = ipc::send(&payload.to_string(), pipe)?;
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(resp.lines().next().unwrap_or("")) {
        if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
            if err != "success" {
                return Err(format!("mpv: {err}"));
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn warmup(state: tauri::State<'_, SharedState>) -> Result<(), String> {
    state.player.lock().warmup()
}

#[tauri::command]
pub fn play(
    state: tauri::State<'_, SharedState>,
    video: Video,
    set_queue: bool,
    audio_only: bool,
) -> Result<(), String> {
    *state.audio_only.lock() = audio_only;
    if set_queue {
        state.queue.lock().play_now(video.clone());
    }
    state.set_last_video(video.clone(), audio_only);
    play_cached(&state, &video, audio_only)
}

#[tauri::command]
pub fn stop(state: tauri::State<'_, SharedState>) -> Result<(), String> {
    state.player.lock().stop()
}

#[tauri::command]
pub fn next(state: tauri::State<'_, SharedState>) -> Result<Option<Video>, String> {
    let audio_only = *state.audio_only.lock();
    if let Some(video) = state.queue.lock().next() {
        play_cached(&state, &video, audio_only)?;
        return Ok(Some(video));
    }
    Ok(None)
}

#[tauri::command]
pub fn prev(state: tauri::State<'_, SharedState>) -> Result<Option<Video>, String> {
    let audio_only = *state.audio_only.lock();
    if let Some(video) = state.queue.lock().prev() {
        play_cached(&state, &video, audio_only)?;
        return Ok(Some(video));
    }
    Ok(None)
}

#[tauri::command]
pub fn get_volume(state: tauri::State<'_, SharedState>) -> Result<f64, String> {
    state.player.lock().volume()
}

#[tauri::command]
pub fn set_volume(state: tauri::State<'_, SharedState>, level: f64) -> Result<(), String> {
    state.player.lock().set_volume(level)
}

#[tauri::command]
pub fn sync_video_panel(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, SharedState>,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Result<(), String> {
    let sf = window.scale_factor().map_err(|e| e.to_string())?;
    let owner = crate::video_embed::hwnd_from_window(&window)?;
    let (sx, sy, w, h) = crate::video_embed::screen_rect(
        owner,
        (x * sf).round() as i32,
        (y * sf).round() as i32,
        (width * sf).round() as i32,
        (height * sf).round() as i32,
    );
    state.player.lock().set_video_area(sx, sy, w, h);
    Ok(())
}

#[tauri::command]
pub fn hide_video_panel(state: tauri::State<'_, SharedState>) -> Result<(), String> {
    state.player.lock().clear_video_area();
    Ok(())
}

#[tauri::command]
pub fn prewarm_playlist(
    state: tauri::State<'_, SharedState>,
    items: Vec<Video>,
    audio_only: bool,
) -> Result<(), String> {
    state
        .stream_cache
        .prewarm_async(state.cookies(), items, audio_only);
    Ok(())
}

#[tauri::command]
pub fn prewarm_status(state: tauri::State<'_, SharedState>) -> crate::stream::PrewarmStatus {
    let (done, total) = state.stream_cache.prewarm_status();
    crate::stream::PrewarmStatus { done, total }
}
