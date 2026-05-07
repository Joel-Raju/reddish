use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum FormatHint {
    Json,
    String,
    Blob,
    Int,
    Float,
}

impl fmt::Display for FormatHint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatHint::Json => write!(f, "json"),
            FormatHint::String => write!(f, "string"),
            FormatHint::Blob => write!(f, "blob"),
            FormatHint::Int => write!(f, "int"),
            FormatHint::Float => write!(f, "float"),
        }
    }
}

/// Detect the format of raw bytes.
pub fn detect_format(data: &[u8]) -> FormatHint {
    if data.is_empty() {
        return FormatHint::String;
    }

    // Try JSON first
    if serde_json::from_slice::<serde_json::Value>(data).is_ok() {
        return FormatHint::Json;
    }

    // Check if all bytes are printable ASCII
    if data.iter().all(|&b| b.is_ascii_graphic() || b == b' ' || b == b'\n' || b == b'\r' || b == b'\t') {
        // Check if it's a valid integer
        if let Ok(s) = std::str::from_utf8(data) {
            let trimmed = s.trim();
            if !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_digit() || c == '-') {
                return FormatHint::Int;
            }
            // Check if it's a valid float
            if !trimmed.is_empty() {
                let mut chars = trimmed.chars();
                if chars.all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c == 'e' || c == 'E' || c == '+') {
                    if trimmed.contains('.') || trimmed.contains('e') || trimmed.contains('E') {
                        return FormatHint::Float;
                    }
                }
            }
            return FormatHint::String;
        }
    }

    FormatHint::Blob
}

/// Convert raw data to a string representation based on the format hint.
pub fn stringify(data: &[u8], hint: FormatHint) -> String {
    match hint {
        FormatHint::Json => {
            String::from_utf8_lossy(data).to_string()
        }
        FormatHint::String | FormatHint::Int | FormatHint::Float => {
            String::from_utf8_lossy(data).to_string()
        }
        FormatHint::Blob => {
            // For binary data, show hex representation
            data.iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" ")
        }
    }
}

/// Create a preview string, truncated to max_len characters.
pub fn preview(data: &[u8], max_len: usize, hint: FormatHint) -> String {
    let s = stringify(data, hint);
        if s.len() > max_len {
            let visible_len = max_len.saturating_sub(3);
            let mut preview = s.chars().take(visible_len).collect::<String>();
            preview.push_str("...");
            preview
    } else {
        s
    }
}
