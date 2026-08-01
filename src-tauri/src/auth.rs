use crate::deps::{find_ytdlp, hidden_cmd};
use crate::state::AppState;
use keyring::Entry;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::State;

const SERVICE: &str = "promptub";
const USER: &str = "youtube-session";

const SUCCESS_HTML: &str = r#"<!DOCTYPE html><html lang="pt-BR"><head><meta charset="utf-8"><title>promptub</title>
<style>body{font-family:system-ui;background:#0f0f0f;color:#f1f1f1;display:flex;align-items:center;justify-content:center;min-height:100vh;margin:0}
.card{text-align:center;padding:2rem 3rem;border-radius:12px;background:#212121;max-width:420px}h1{color:#1ed760}p{color:#aaa}</style></head>
<body><div class="card"><h1>promptub</h1><p>Conta conectada. Pode fechar esta aba.</p></div></body></html>"#;

#[tauri::command]
pub fn is_logged_in() -> bool {
    has_session_cookies()
}

#[tauri::command]
pub fn has_premium_session() -> bool {
    has_session_cookies()
}

fn has_session_cookies() -> bool {
    Entry::new(SERVICE, USER)
        .and_then(|e| e.get_password())
        .map(|s| !s.is_empty())
        .unwrap_or(false)
}

#[tauri::command]
pub fn login(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    if is_logged_in() {
        return apply_session(&state);
    }
    let browser = std::env::var("PROMPTUB_BROWSER").unwrap_or_else(|_| "edge".into());
    let cookies = run_login_flow(&browser)?;
    Entry::new(SERVICE, USER)
        .map_err(|e| e.to_string())?
        .set_password(&cookies)
        .map_err(|e| e.to_string())?;
    apply_session(&state)
}

#[tauri::command]
pub fn logout(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    if let Ok(entry) = Entry::new(SERVICE, USER) {
        let _ = entry.delete_credential();
    }
    state.set_cookies(String::new());
    state.player.lock().set_cookies(String::new());
    Ok(())
}

pub fn load_cookies(state: &Arc<AppState>) {
    if let Ok(cookies) = Entry::new(SERVICE, USER).and_then(|e| e.get_password()) {
        if let Ok(path) = write_session_file(&cookies) {
            state.set_cookies(path.clone());
            state.player.lock().set_cookies(path);
        }
    }
}

fn apply_session(state: &Arc<AppState>) -> Result<(), String> {
    let cookies = Entry::new(SERVICE, USER)
        .map_err(|e| e.to_string())?
        .get_password()
        .map_err(|e| e.to_string())?;
    let path = write_session_file(&cookies)?;
    state.set_cookies(path.clone());
    state.player.lock().set_cookies(path);
    Ok(())
}

fn run_login_flow(browser: &str) -> Result<String, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();

    let (done_tx, done_rx) = mpsc::channel::<()>();
    let browser_owned = browser.to_string();

    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            if handle_request(stream, port).is_some() {
                let _ = done_tx.send(());
                break;
            }
        }
    });

    let start_url = format!("http://127.0.0.1:{port}/");
    open_browser(&start_url)?;

    let deadline = Duration::from_secs(10 * 60);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > deadline {
            return Err("Tempo esgotado — conclua o login no navegador".into());
        }

        if done_rx.try_recv().is_ok() {
            thread::sleep(Duration::from_secs(2));
            if let Ok(c) = extract_cookies(browser) {
                return Ok(c);
            }
        }

        if let Ok(c) = extract_cookies(&browser_owned) {
            return Ok(c);
        }

        thread::sleep(Duration::from_secs(5));
    }
}

fn handle_request(mut stream: TcpStream, port: u16) -> Option<()> {
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf).ok()?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let path = req.lines().next()?.split_whitespace().nth(1)?;

    if path.starts_with("/done") {
        respond_html(stream, SUCCESS_HTML);
        return Some(());
    }

    let google = "https://accounts.google.com/ServiceLogin?service=youtube&passive=false&continue=https%3A%2F%2Fwww.youtube.com%2Fsignin%3Faction_handle_signin%3Dtrue%26app%3Ddesktop%26hl%3Dpt%26next%3D%252F";
    let html = format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"><meta http-equiv="refresh" content="1;url={google}"></head>
<body style="background:#0f0f0f;color:#aaa;font-family:system-ui;text-align:center;padding:3rem">
<p>Redirecionando para login…</p>
<p style="font-size:.85rem;margin-top:2rem">Depois de entrar, visite <a href="http://127.0.0.1:{port}/done" style="color:#1ed760">concluir login</a></p>
</body></html>"#
    );
    respond_html(stream, &html);
    None
}

fn respond_html(mut stream: TcpStream, body: &str) {
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes());
}

fn open_browser(url: &str) -> Result<(), String> {
    hidden_cmd("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn()
        .map_err(|e| format!("abrir navegador: {e}"))?;
    Ok(())
}

fn extract_cookies(browser: &str) -> Result<String, String> {
    let tmp = std::env::temp_dir().join(format!("promptub-export-{}.txt", std::process::id()));
    let ytdlp = find_ytdlp().ok_or("yt-dlp não encontrado")?;
    let output = hidden_cmd(&ytdlp)
        .args([
            "--cookies-from-browser",
            browser,
            "--cookies",
            tmp.to_str().ok_or("caminho inválido")?,
            "--skip-download",
            "https://www.youtube.com/robots.txt",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let content = fs::read_to_string(&tmp).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&tmp);
    if !has_login_cookie(&content) {
        return Err("login não detectado".into());
    }
    Ok(content)
}

fn has_login_cookie(content: &str) -> bool {
    content.contains("LOGIN_INFO")
        || (content.contains("SID") && content.contains("youtube.com"))
}

fn write_session_file(cookies: &str) -> Result<String, String> {
    let dir = dirs_session_dir()?;
    let path = dir.join("cookies.txt");
    fs::write(&path, cookies).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into())
}

fn dirs_session_dir() -> Result<std::path::PathBuf, String> {
    let base = std::env::var("APPDATA").map_err(|e| e.to_string())?;
    let dir = std::path::PathBuf::from(base).join("promptub");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}
