use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

static BUNDLED_YTDLP: OnceLock<Option<String>> = OnceLock::new();
static BUNDLED_MPV: OnceLock<Option<String>> = OnceLock::new();

pub fn init_bundled_tools(tools_dir: Option<PathBuf>) {
    let (ytdlp, mpv) = match tools_dir {
        Some(dir) => (tool_in_dir(&dir, "yt-dlp"), tool_in_dir(&dir, "mpv")),
        None => (None, None),
    };
    let _ = BUNDLED_YTDLP.set(ytdlp);
    let _ = BUNDLED_MPV.set(mpv);
}

fn tool_in_dir(dir: &Path, name: &str) -> Option<String> {
    let exe = dir.join(format!("{name}.exe"));
    if exe.is_file() {
        Some(exe.to_string_lossy().into_owned())
    } else {
        None
    }
}

#[tauri::command]
pub fn check_deps() -> Result<(), String> {
    find_ytdlp().ok_or_else(|| {
        "yt-dlp nao encontrado. Reinstale o promptub ou execute o instalador completo.".to_string()
    })?;
    find_mpv().ok_or_else(|| {
        "mpv nao encontrado. Reinstale o promptub ou execute o instalador completo.".to_string()
    })?;
    Ok(())
}

pub fn find_ytdlp() -> Option<String> {
    bundled(BUNDLED_YTDLP.get()).or_else(|| which("yt-dlp"))
}

pub fn find_mpv() -> Option<String> {
    if let Some(p) = bundled(BUNDLED_MPV.get()) {
        return Some(p);
    }
    if let Some(p) = which("mpv") {
        return Some(p);
    }
    for candidate in mpv_windows_paths() {
        if Path::new(&candidate).exists() {
            return Some(candidate);
        }
    }
    None
}

fn bundled(slot: Option<&Option<String>>) -> Option<String> {
    slot.and_then(|p| p.clone())
}

fn which(name: &str) -> Option<String> {
    let output = hidden_cmd("where").arg(name).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()?
        .trim()
        .to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

fn mpv_windows_paths() -> Vec<String> {
    let mut paths = Vec::new();
    if let Ok(pf) = std::env::var("ProgramFiles") {
        paths.push(format!(r"{pf}\MPV Player\mpv.exe"));
        paths.push(format!(r"{pf}\mpv\mpv.exe"));
    }
    paths
}

pub fn mpv_cmd(program: &str) -> Command {
    let mut cmd = hidden_cmd(program);
    if let Some(dir) = Path::new(program).parent() {
        if dir.is_dir() {
            cmd.current_dir(dir);
        }
    }
    cmd
}

pub fn utf8_cmd(program: &str) -> Command {
    let mut cmd = hidden_cmd(program);
    cmd.env("PYTHONIOENCODING", "utf-8");
    cmd.env("PYTHONUTF8", "1");
    cmd.env("PYTHONLEGACYWINDOWSSTDIO", "0");
    cmd.env("LC_ALL", "C.UTF-8");
    cmd
}

pub fn hidden_cmd(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}
