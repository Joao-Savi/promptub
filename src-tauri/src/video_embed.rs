#[cfg(windows)]
mod imp {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use winapi::shared::windef::{HWND, POINT};
    use winapi::um::libloaderapi::GetModuleHandleW;
    use winapi::um::winuser::{
        ClientToScreen, CreateWindowExW, DestroyWindow, MoveWindow, RegisterClassExW,
        SetWindowPos, ShowWindow, CS_HREDRAW, CS_VREDRAW, HWND_NOTOPMOST, HWND_TOPMOST, SW_HIDE,
        SW_SHOW, SWP_NOACTIVATE, SWP_SHOWWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
        WS_VISIBLE, WNDCLASSEXW,
    };

    const CLASS_NAME: &str = "PromptubVideoPane";

    pub struct VideoOverlay {
        owner: isize,
        child: isize,
        ready: bool,
        visible: bool,
    }

    impl VideoOverlay {
        pub fn new() -> Self {
            Self {
                owner: 0,
                child: 0,
                ready: false,
                visible: false,
            }
        }

        pub fn sync(&mut self, owner: isize, x: i32, y: i32, w: i32, h: i32) -> isize {
            if owner == 0 {
                return 0;
            }
            self.ensure_class();
            let width = w.max(320);
            let height = h.max(180);
            let (screen_x, screen_y) = client_to_screen(owner, x, y);

            if self.child == 0 || self.owner != owner {
                self.destroy();
                self.owner = owner;
                self.child = unsafe { create_popup(owner, screen_x, screen_y, width, height) };
            } else {
                unsafe {
                    MoveWindow(self.child as HWND, screen_x, screen_y, width, height, 1);
                    raise_popup(self.child as HWND, screen_x, screen_y, width, height);
                    ShowWindow(self.child as HWND, SW_SHOW);
                }
            }
            self.visible = true;
            self.child
        }

        pub fn hide(&mut self) {
            if self.child != 0 {
                unsafe {
                    ShowWindow(self.child as HWND, SW_HIDE);
                    SetWindowPos(
                        self.child as HWND,
                        HWND_NOTOPMOST,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOACTIVATE,
                    );
                }
            }
            self.visible = false;
        }

        pub fn destroy(&mut self) {
            if self.child != 0 {
                unsafe {
                    DestroyWindow(self.child as HWND);
                }
                self.child = 0;
            }
            self.visible = false;
        }

        fn ensure_class(&mut self) {
            if self.ready {
                return;
            }
            unsafe {
                use std::mem::zeroed;
                let class_name = wide(CLASS_NAME);
                let mut wc: WNDCLASSEXW = zeroed();
                wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
                wc.style = CS_HREDRAW | CS_VREDRAW;
                wc.hInstance = GetModuleHandleW(std::ptr::null());
                wc.lpszClassName = class_name.as_ptr();
                RegisterClassExW(&wc);
            }
            self.ready = true;
        }
    }

    impl Drop for VideoOverlay {
        fn drop(&mut self) {
            self.destroy();
        }
    }

    fn client_to_screen(owner: isize, x: i32, y: i32) -> (i32, i32) {
        let mut pt = POINT { x, y };
        unsafe {
            ClientToScreen(owner as HWND, &mut pt);
        }
        (pt.x, pt.y)
    }

    unsafe fn raise_popup(hwnd: HWND, x: i32, y: i32, w: i32, h: i32) {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            y,
            w,
            h,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }

    unsafe fn create_popup(owner: isize, x: i32, y: i32, w: i32, h: i32) -> isize {
        let class_name = wide(CLASS_NAME);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class_name.as_ptr(),
            std::ptr::null(),
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            w,
            h,
            owner as HWND,
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null_mut(),
        );
        if !hwnd.is_null() {
            raise_popup(hwnd, x, y, w, h);
        }
        hwnd as isize
    }

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }
}

#[cfg(windows)]
pub use imp::VideoOverlay;

#[cfg(not(windows))]
pub struct VideoOverlay;

#[cfg(not(windows))]
impl VideoOverlay {
    pub fn new() -> Self {
        Self
    }
    pub fn sync(&mut self, _owner: isize, _x: i32, _y: i32, _w: i32, _h: i32) -> isize {
        0
    }
    pub fn hide(&mut self) {}
    pub fn destroy(&mut self) {}
}

pub fn hwnd_from_window(window: &tauri::WebviewWindow) -> Result<isize, String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = window.window_handle().map_err(|e| e.to_string())?;
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => Ok(h.hwnd.get() as isize),
        _ => Err("overlay de video so no Windows".into()),
    }
}
