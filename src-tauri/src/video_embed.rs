#[cfg(windows)]
mod imp {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::sync::Mutex;
    use tauri::WebviewWindow;
    use winapi::shared::minwindef::{BOOL, LPARAM, TRUE};
    use winapi::shared::windef::HWND;
    use winapi::shared::windef::POINT;
    use winapi::um::winuser::{
        CreateWindowExW, DestroyWindow, EnumChildWindows, GetWindowRect, RegisterClassExW,
        ScreenToClient, SetWindowPos, ShowWindow, CS_HREDRAW, CS_VREDRAW, HWND_TOP,
        SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW, WNDCLASSEXW, WS_CHILD, WS_CLIPSIBLINGS,
        WS_VISIBLE,
    };
    use winapi::um::wingdi::{GetStockObject, BLACK_BRUSH};

    static HOST: Mutex<Option<VideoHost>> = Mutex::new(None);

    pub struct VideoHost {
        host: isize,
        parent: isize,
    }

    unsafe extern "system" fn enum_largest_child(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam as *mut EnumCtx);
        if hwnd == ctx.skip {
            return TRUE;
        }
        if GetWindowRect(hwnd, &mut ctx.rect) == 0 {
            return TRUE;
        }
        let w = ctx.rect.right - ctx.rect.left;
        let h = ctx.rect.bottom - ctx.rect.top;
        let area = w * h;
        if area > ctx.best_area {
            ctx.best_area = area;
            ctx.best = hwnd;
        }
        TRUE
    }

    struct EnumCtx {
        skip: HWND,
        best: HWND,
        best_area: i32,
        rect: winapi::shared::windef::RECT,
    }

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    fn parent_hwnd(window: &WebviewWindow) -> Result<HWND, String> {
        let handle = window.window_handle().map_err(|e| e.to_string())?;
        match handle.as_raw() {
            RawWindowHandle::Win32(h) => Ok(h.hwnd.get() as HWND),
            _ => Err("HWND indisponivel".into()),
        }
    }

    unsafe fn webview_sibling(parent: HWND, skip: HWND) -> Option<HWND> {
        let mut ctx = EnumCtx {
            skip,
            best: std::ptr::null_mut(),
            best_area: 0,
            rect: std::mem::zeroed(),
        };
        EnumChildWindows(
            parent,
            Some(enum_largest_child),
            &mut ctx as *mut _ as LPARAM,
        );
        if ctx.best.is_null() {
            None
        } else {
            Some(ctx.best)
        }
    }

    fn screen_to_client(parent: HWND, screen_x: i32, screen_y: i32) -> Result<(i32, i32), String> {
        let mut pt = POINT {
            x: screen_x,
            y: screen_y,
        };
        unsafe {
            if ScreenToClient(parent, &mut pt) == 0 {
                return Err("ScreenToClient falhou".into());
            }
        }
        Ok((pt.x, pt.y))
    }

    fn ensure_class() -> Result<Vec<u16>, String> {
        static REGISTERED: std::sync::Once = std::sync::Once::new();

        REGISTERED.call_once(|| unsafe {
            let name = wide("PromptubVideoHost");
            let mut wc: WNDCLASSEXW = std::mem::zeroed();
            wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
            wc.lpfnWndProc = Some(winapi::um::winuser::DefWindowProcW);
            wc.hInstance = winapi::um::libloaderapi::GetModuleHandleW(std::ptr::null());
            wc.lpszClassName = name.as_ptr();
            wc.style = CS_HREDRAW | CS_VREDRAW;
            wc.hbrBackground = GetStockObject(BLACK_BRUSH as i32) as winapi::shared::windef::HBRUSH;
            let _ = RegisterClassExW(&wc);
        });

        Ok(wide("PromptubVideoHost"))
    }

    pub fn ensure_host(
        window: &WebviewWindow,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    ) -> Result<isize, String> {
        let sf = window.scale_factor().map_err(|e| e.to_string())?;
        let inner = window.inner_position().map_err(|e| e.to_string())?;
        let width = ((w * sf).round() as i32).max(2);
        let height = ((h * sf).round() as i32).max(2);

        let screen_x = inner.x + (x * sf).round() as i32;
        let screen_y = inner.y + (y * sf).round() as i32;

        let parent = parent_hwnd(window)?;
        let (cx, cy) = screen_to_client(parent, screen_x, screen_y)?;

        let mut guard = HOST.lock().map_err(|_| "host lock".to_string())?;
        let host_hwnd = if let Some(ref host) = *guard {
            if host.parent == parent as isize {
                host.host as HWND
            } else {
                unsafe {
                    DestroyWindow(host.host as HWND);
                }
                create_host(parent)?
            }
        } else {
            create_host(parent)?
        };

        unsafe {
            let insert_after = webview_sibling(parent, host_hwnd).unwrap_or(HWND_TOP);
            SetWindowPos(
                host_hwnd,
                insert_after,
                cx,
                cy,
                width,
                height,
                SWP_SHOWWINDOW | SWP_NOACTIVATE,
            );
            ShowWindow(host_hwnd, SW_SHOW);
        }

        *guard = Some(VideoHost {
            host: host_hwnd as isize,
            parent: parent as isize,
        });

        Ok(host_hwnd as isize)
    }

    fn create_host(parent: HWND) -> Result<HWND, String> {
        let class = ensure_class()?;
        unsafe {
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                std::ptr::null(),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                0,
                0,
                320,
                180,
                parent,
                std::ptr::null_mut(),
                winapi::um::libloaderapi::GetModuleHandleW(std::ptr::null()),
                std::ptr::null_mut(),
            );
            if hwnd.is_null() {
                return Err("falha ao criar host de video".into());
            }
            Ok(hwnd)
        }
    }

    pub fn set_host_visible(visible: bool) {
        if let Ok(guard) = HOST.lock() {
            if let Some(ref host) = *guard {
                unsafe {
                    ShowWindow(
                        host.host as HWND,
                        if visible { SW_SHOW } else { SW_HIDE },
                    );
                }
            }
        }
    }

    pub fn clear_host() {
        if let Ok(mut guard) = HOST.lock() {
            if let Some(host) = guard.take() {
                unsafe {
                    DestroyWindow(host.host as HWND);
                }
            }
        }
    }

    pub fn client_to_screen(owner: isize, x: i32, y: i32) -> (i32, i32) {
        use winapi::shared::windef::POINT;
        use winapi::um::winuser::ClientToScreen;
        let mut pt = POINT { x, y };
        unsafe {
            ClientToScreen(owner as HWND, &mut pt);
        }
        (pt.x, pt.y)
    }

    pub fn hwnd_from_window(window: &WebviewWindow) -> Result<isize, String> {
        Ok(parent_hwnd(window)? as isize)
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
}

#[cfg(windows)]
pub use imp::*;

#[cfg(not(windows))]
use tauri::WebviewWindow;

#[cfg(not(windows))]
pub fn ensure_host(
    _window: &WebviewWindow,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> Result<isize, String> {
    Ok(((x as i64) << 32 | (y as i64 & 0xffff)) as isize)
}

#[cfg(not(windows))]
pub fn set_host_visible(_visible: bool) {}

#[cfg(not(windows))]
pub fn clear_host() {}

#[cfg(not(windows))]
pub fn client_to_screen(_owner: isize, x: i32, y: i32) -> (i32, i32) {
    (x, y)
}

#[cfg(not(windows))]
pub fn hwnd_from_window(_window: &WebviewWindow) -> Result<isize, String> {
    Err("video embed so no Windows".into())
}

#[cfg(not(windows))]
pub fn screen_rect(
    _owner: isize,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> (i32, i32, i32, i32) {
    (x, y, width.max(320), height.max(180))
}
