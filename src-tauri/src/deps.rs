use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

static BUNDLED_YTDLP: OnceLock<Option<String>> = OnceLock::new();

pub fn init_bundled_tools(tools_dir: Option<PathBuf>) {
    let mut dirs: Vec<PathBuf> = tools_dir.into_iter().collect();
    dirs.extend(local_tool_dirs());
    let ytdlp = dirs.iter().find_map(|d| tool_in_dir(d, "yt-dlp"));
    let _ = BUNDLED_YTDLP.set(ytdlp);
}

fn local_tool_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let Ok(exe) = std::env::current_exe() else {
        return dirs;
    };
    let Some(parent) = exe.parent() else {
        return dirs;
    };
    dirs.push(parent.join("tools"));
    dirs.push(parent.join("resources").join("tools"));
    dirs.push(
        parent
            .join("..")
            .join("src-tauri")
            .join("resources")
            .join("tools"),
    );
    dirs.push(
        parent
            .join("..")
            .join("src-tauri")
            .join("target")
            .join("release")
            .join("tools"),
    );
    dirs
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
    Ok(())
}

pub fn find_ytdlp() -> Option<String> {
    which("yt-dlp")
        .or_else(|| bundled(BUNDLED_YTDLP.get()))
        .or_else(|| local_tool_dirs().iter().find_map(|d| tool_in_dir(d, "yt-dlp")))
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
