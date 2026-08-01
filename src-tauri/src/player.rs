use crate::deps::{find_mpv, find_ytdlp, mpv_cmd};
use crate::ipc;
use crate::queue_refill;
use crate::state::SharedState;
use crate::youtube::Video;
use serde_json::json;
use std::io::{Read, Write};
use std::process::Child;
use std::thread;
use std::time::Duration;

const FORMAT_AUDIO: &str = "bestaudio[ext=m4a]/bestaudio/best";

pub fn video_format_for_quality(quality: &str) -> &'static str {
    match quality {
        "360" => "b[height<=360]/best[height<=360]/bestvideo[height<=360]+bestaudio/best",
        "480" => "b[height<=480]/best[height<=480]/bestvideo[height<=480]+bestaudio/best",
        "720" => "b[height<=720]/best[height<=720]/bestvideo[height<=720]+bestaudio/best",
        "1080" => {
            "b[height<=1080]/best[height<=1080]/bestvideo[height<=1080]+bestaudio/best"
        }
        "best" => "bestvideo+bestaudio/best",
        _ => "b[height<=720]/best[height<=720]/bestvideo[height<=720]+bestaudio/best",
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MpvWindow {
    Audio,
    Video { wid: isize, w: i32, h: i32 },
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

    pub fn set_video_area(&mut self, wid: isize, w: i32, h: i32) {
        let next = MpvWindow::Video { wid, w, h };
        let needs_restart = match self.mpv_window {
            MpvWindow::Audio => true,
            MpvWindow::Video {
                wid: ow,
                w: ow_w,
                h: oh_h,
            } => ow != wid || (ow_w - w).abs() > 2 || (oh_h - h).abs() > 2,
        };
        self.mpv_window = next;
        if needs_restart && self.daemon_responding() {
            let _ = self.restart();
        }
    }

    pub fn clear_video_area(&mut self) {
        if self.mpv_window == MpvWindow::Audio {
            return;
        }
        self.mpv_window = MpvWindow::Audio;
        crate::video_embed::clear_host();
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
        video_quality: &str,
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
        } else {
            ipc_property(&self.pipe_name, "force-window", json!(true))?;
            let _ = ipc_property(&self.pipe_name, "keepaspect-window", json!(true));
            let _ = ipc_property(&self.pipe_name, "video-unscaled", json!(false));
            let _ = ipc_property(&self.pipe_name, "panscan", json!(0.0));
        }

        let client = if audio_only { "android" } else { "web,default" };
        let ytdl_raw = self.ytdl_raw_options(client);
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
            let format = if audio_only {
                FORMAT_AUDIO
            } else {
                video_format_for_quality(video_quality)
            };
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
        let audio_mode = matches!(self.mpv_window, MpvWindow::Audio);
        let client = if audio_mode { "android" } else { "web,default" };
        let ytdl_raw = self.ytdl_raw_options(client);

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

        if let Some(ytdlp) = find_ytdlp() {
            args.push(format!("--ytdl-program={ytdlp}"));
        }

        match self.mpv_window {
            MpvWindow::Audio => {
                args.push("--force-window=no".to_string());
                args.push("--no-video".to_string());
            }
            MpvWindow::Video { wid, .. } => {
                args.push(format!("--wid={wid}"));
                args.push("--force-window=immediate".to_string());
                args.push("--keepaspect-window=yes".to_string());
                args.push("--video-unscaled=no".to_string());
                args.push("--no-border".to_string());
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
        crate::video_embed::clear_host();
        thread::sleep(Duration::from_millis(100));
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub(crate) fn play_cached(state: &SharedState, video: &Video, audio_only: bool) -> Result<(), String> {
    if !audio_only {
        crate::video_embed::clear_host();
        let mut player = state.player.lock();
        player.clear_video_area();
        let _ = player.stop();
        return Ok(());
    }
    let direct = state.stream_cache.get(&video.id);
    let quality = state.video_quality.lock().clone();
    state
        .player
        .lock()
        .play(video, true, direct.as_deref(), &quality)
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
                                crate::stream::prewarm_queue_ahead(&state);
                                queue_refill::maybe_refill_queue(&state);
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
pub async fn resolve_stream(
    state: tauri::State<'_, SharedState>,
    video_id: String,
    video_url: Option<String>,
) -> Result<String, String> {
    let id = video_id.trim().to_string();
    if id.is_empty() {
        return Err("ID de video invalido".into());
    }
    if let Some(cached) = state.stream_cache.get(&id) {
        return Ok(cached);
    }
    let cookies = state.cookies();
    let quality = state.video_quality.lock().clone();
    let audio_only = *state.audio_only.lock();
    let id_fetch = id.clone();
    let cookies_fetch = cookies.clone();
    let url = tauri::async_runtime::spawn_blocking(move || -> Result<String, String> {
        let video = video_for_stream(&cookies_fetch, &id_fetch, video_url.as_deref())?;
        crate::stream::resolve_stream_url_with_quality(&cookies_fetch, &video, audio_only, &quality)
    })
    .await
    .map_err(|e| format!("resolve_stream: {e}"))??;
    state.stream_cache.put(id, url.clone());
    Ok(url)
}

fn video_for_stream(cookies: &str, id: &str, video_url: Option<&str>) -> Result<Video, String> {
    if let Some(url) = video_url.map(str::trim).filter(|u| !u.is_empty()) {
        return Ok(Video {
            id: id.to_string(),
            title: String::new(),
            uploader: String::new(),
            duration: String::new(),
            url: url.to_string(),
            thumbnail: String::new(),
            is_live: false,
        });
    }
    crate::youtube::fetch_video(cookies, id)?.ok_or_else(|| "Video nao encontrado".into())
}

#[tauri::command]
pub fn prewarm_streams(
    state: tauri::State<'_, SharedState>,
    items: Vec<Video>,
) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }
    let quality = state.video_quality.lock().clone();
    state
        .stream_cache
        .prewarm_async(state.cookies(), items, false, quality);
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
    state.watch_history.lock().record(video.clone(), audio_only);
    play_cached(&state, &video, audio_only)?;
    crate::stream::prewarm_queue_ahead(&state);
    queue_refill::maybe_refill_queue(&state);
    Ok(())
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
        crate::stream::prewarm_queue_ahead(&state);
        queue_refill::maybe_refill_queue(&state);
        return Ok(Some(video));
    }
    Ok(None)
}

#[tauri::command]
pub fn prev(state: tauri::State<'_, SharedState>) -> Result<Option<Video>, String> {
    let audio_only = *state.audio_only.lock();
    if let Some(video) = state.queue.lock().prev() {
        play_cached(&state, &video, audio_only)?;
        crate::stream::prewarm_queue_ahead(&state);
        queue_refill::maybe_refill_queue(&state);
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
    _window: tauri::WebviewWindow,
    state: tauri::State<'_, SharedState>,
    _x: f64,
    _y: f64,
    _width: f64,
    _height: f64,
) -> Result<(), String> {
    // Legado: overlay mpv desativado — video roda no <video> HTML5.
    crate::video_embed::clear_host();
    state.player.lock().clear_video_area();
    Ok(())
}

#[tauri::command]
pub fn set_video_overlay_visible(_visible: bool) -> Result<(), String> {
    crate::video_embed::clear_host();
    Ok(())
}

#[tauri::command]
pub fn hide_video_panel(state: tauri::State<'_, SharedState>) -> Result<(), String> {
    state.player.lock().clear_video_area();
    Ok(())
}

#[tauri::command]
pub fn get_video_quality(state: tauri::State<'_, SharedState>) -> String {
    state.video_quality.lock().clone()
}

#[tauri::command]
pub fn set_video_quality(
    state: tauri::State<'_, SharedState>,
    quality: String,
) -> Result<(), String> {
    let q = quality.trim();
    if !["360", "480", "720", "1080", "best"].contains(&q) {
        return Err("Qualidade invalida (360, 480, 720, 1080, best)".into());
    }
    *state.video_quality.lock() = q.to_string();
    let audio_only = *state.audio_only.lock();
    if !audio_only {
        if let Some(v) = state
            .queue
            .lock()
            .current_video()
            .or_else(|| state.last_video.lock().clone())
        {
            state.stream_cache.remove(&v.id);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn prewarm_playlist(
    state: tauri::State<'_, SharedState>,
    items: Vec<Video>,
    audio_only: bool,
) -> Result<(), String> {
    let _ = items;
    let _ = audio_only;
    crate::stream::prewarm_queue_ahead(&state);
    Ok(())
}

#[tauri::command]
pub fn prewarm_status(state: tauri::State<'_, SharedState>) -> crate::stream::PrewarmStatus {
    let (done, total) = state.stream_cache.prewarm_status();
    crate::stream::PrewarmStatus { done, total }
}
