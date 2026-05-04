use base64::{engine::general_purpose::STANDARD as BASE64_ENGINE, Engine};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatHint {
    Json,
    Base64,
    Number,
    RawString,
    Blob,
}

pub fn detect_format(raw: &[u8]) -> FormatHint {
    if raw.len() > 64 && is_base64ish(raw) {
        return FormatHint::Base64;
    }
    if is_valid_json(raw) {
        return FormatHint::Json;
    }
    if raw.iter().all(|b| b.is_ascii_digit() || *b == b'.' || *b == b'-') && !raw.is_empty() {
        return FormatHint::Number;
    }
    if raw.iter().any(|b| b.is_ascii_control() && *b != b'\n' && *b != b'\r' && *b != b'\t') {
        return FormatHint::Blob;
    }
    FormatHint::RawString
}

fn is_base64ish(raw: &[u8]) -> bool {
    raw.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'+' || *b == b'/' || *b == b'=')
}

fn is_valid_json(raw: &[u8]) -> bool {
    std::str::from_utf8(raw)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .is_some()
}

fn b64enc(data: &[u8]) -> String {
    BASE64_ENGINE.encode(data)
}

pub fn stringify(raw: &[u8], hint: FormatHint) -> String {
    match hint {
        FormatHint::Json => pretty_print_json(raw),
        FormatHint::Blob => b64enc(raw),
        _ => std::str::from_utf8(raw)
            .map(String::from)
            .unwrap_or_else(|_| b64enc(raw)),
    }
}

pub fn preview(raw: &[u8], max_chars: usize, hint: FormatHint) -> String {
    let s = stringify(raw, hint);
    if s.chars().count() <= max_chars {
        s
    } else {
        s.chars().take(max_chars.saturating_sub(3)).collect::<String>() + "..."
    }
}

pub fn pretty_print_json(raw: &[u8]) -> String {
    std::str::from_utf8(raw)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| String::from_utf8_lossy(raw).to_string())
}
