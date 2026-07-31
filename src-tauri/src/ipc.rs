use std::io::{Read, Write};

#[cfg(windows)]
pub fn send(payload: &str, pipe_name: &str) -> Result<String, String> {
    let mut client = dial(pipe_name)?;
    client
        .write_all(payload.as_bytes())
        .map_err(|e| e.to_string())?;
    client.write_all(b"\n").map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 8192];
    let n = client.read(&mut buf).map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&buf[..n]).to_string())
}

#[cfg(windows)]
pub fn dial(pipe_name: &str) -> Result<std::fs::File, String> {
    use std::fs::File;
    use std::os::windows::io::FromRawHandle;
    use std::os::windows::ffi::OsStrExt;
    use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
    use winapi::um::namedpipeapi::WaitNamedPipeW;
    use winapi::um::winnt::{FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};

    let path = format!(r"\\.\pipe\{pipe_name}");
    let wide: Vec<u16> = std::ffi::OsStr::new(&path).encode_wide().chain(Some(0)).collect();

    unsafe {
        let waited = WaitNamedPipeW(wide.as_ptr(), 5000);
        if waited == 0 {
            return Err(format!("pipe indisponivel: {pipe_name}"));
        }
        let handle = CreateFileW(
            wide.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        );
        if handle == INVALID_HANDLE_VALUE {
            return Err(format!("nao foi possivel abrir pipe: {pipe_name}"));
        }
        Ok(File::from_raw_handle(handle as _))
    }
}

#[cfg(not(windows))]
pub fn send(_payload: &str, _pipe_name: &str) -> Result<String, String> {
    Err("IPC mpv suportado apenas no Windows".into())
}

#[cfg(not(windows))]
pub fn dial(_pipe_name: &str) -> Result<std::fs::File, String> {
    Err("IPC mpv suportado apenas no Windows".into())
}
