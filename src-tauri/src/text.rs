/// Decodifica stdout/stderr de subprocessos no Windows (UTF-8 ou CP1252).
pub fn decode_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return repair_mojibake(s);
    }
    let (decoded, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
    repair_mojibake(&decoded)
}

/// Corrige UTF-8 lido como Latin-1 (ex.: "ZÃ©" -> "Zé").
pub fn repair_mojibake(s: &str) -> String {
    if !s.contains('Ã') && !s.contains('Â') {
        return s.to_string();
    }
    if !s.is_ascii() && !s.contains('Ã') {
        return s.to_string();
    }
    let bytes: Vec<u8> = s.chars().map(|c| c as u8).collect();
    if let Ok(fixed) = std::str::from_utf8(&bytes) {
        if fixed.chars().any(|c| c.is_alphabetic() && c as u32 > 127) {
            return fixed.to_string();
        }
    }
    s.to_string()
}
