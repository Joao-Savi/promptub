#[cfg(windows)]
mod imp {
    use winapi::shared::windef::POINT;
    use winapi::um::winuser::ClientToScreen;

    pub fn client_to_screen(owner: isize, x: i32, y: i32) -> (i32, i32) {
        let mut pt = POINT { x, y };
        unsafe {
            ClientToScreen(owner as winapi::shared::windef::HWND, &mut pt);
        }
        (pt.x, pt.y)
    }
}

#[cfg(windows)]
pub use imp::client_to_screen;

#[cfg(not(windows))]
pub fn client_to_screen(_owner: isize, x: i32, y: i32) -> (i32, i32) {
    (x, y)
}

pub fn hwnd_from_window(window: &tauri::WebviewWindow) -> Result<isize, String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = window.window_handle().map_err(|e| e.to_string())?;
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => Ok(h.hwnd.get() as isize),
        _ => Err("video so no Windows".into()),
    }
}

pub fn screen_rect(
    owner: isize,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> (i32, i32, i32, i32) {
    let (sx, sy) = client_to_screen(owner, x, y);
    (sx, sy, width.max(320), height.max(180))
}
