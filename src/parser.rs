use memchr::memchr;
use std::ops::ControlFlow;

#[derive(Debug)]
pub enum Token<'a> {
    StartTag(&'a str, &'a str), // name, attributes
    EndTag(&'a str),
    Text(&'a str),
}

pub use std::ops::ControlFlow::{Break, Continue};

/// Streams tokens from XML without allocations.
/// Calls `on_token` for each parsed token.
pub fn stream_xml<'a, F>(xml: &'a str, mut on_token: F)
where
    F: FnMut(Token<'a>) -> ControlFlow<()>, 
{
    let bytes = xml.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        // Fast skip whitespace using memchr
        while pos < len {
            let b = unsafe { *bytes.get_unchecked(pos) };
            if !b.is_ascii_whitespace() {
                break;
            }
            pos += 1;
        }

        if pos >= len {
            break;
        }

        let current_byte = unsafe { *bytes.get_unchecked(pos) };
        if current_byte == b'<' {
            if pos + 1 < len && unsafe { *bytes.get_unchecked(pos + 1) } == b'/' {
                let start = pos + 2;
                if let Some(rel) = memchr(b'>', &bytes[start..]) {
                    let end_pos = start + rel;
                    let name = unsafe { xml.get_unchecked(start..end_pos) };
                    if on_token(Token::EndTag(name)).is_break() {
                        return;
                    }
                    pos = end_pos + 1;
                } else {
                    break;
                }
            } else if pos + 3 < len && unsafe { *bytes.get_unchecked(pos + 1) } == b'!' {
                let mut end_pos = pos + 2;
                while end_pos < len && unsafe { *bytes.get_unchecked(end_pos) } != b'>' {
                    end_pos += 1;
                }
                pos = if end_pos < len { end_pos + 1 } else { len };
            } else {
                let start = pos + 1;
                if let Some(rel) = memchr(b'>', &bytes[start..]) {
                    let end_pos = start + rel;
                    let is_self_closing = end_pos > start && unsafe { *bytes.get_unchecked(end_pos - 1) } == b'/';
                    let mut name_end = start;
                    while name_end < end_pos {
                        let byte = unsafe { *bytes.get_unchecked(name_end) };
                        if byte <= b' ' || byte == b'/' {
                            break;
                        }
                        name_end += 1;
                    }
                    let name = unsafe { xml.get_unchecked(start..name_end) };
                    
                    let attrs_start = name_end;
                    let attrs_end = if is_self_closing { end_pos - 1 } else { end_pos };
                    let attrs = unsafe { xml.get_unchecked(attrs_start..attrs_end) };

                    if on_token(Token::StartTag(name, attrs)).is_break() {
                        return;
                    }
                    if is_self_closing && on_token(Token::EndTag(name)).is_break() {
                        return;
                    }
                    pos = end_pos + 1;
                } else {
                    break;
                }
            }
        } else {
            let start = pos;
            let end_pos = memchr(b'<', &bytes[start..]).map_or(len, |rel| start + rel);
            if end_pos > start {
                let mut t_start = start;
                let mut t_end = end_pos;
                while t_start < t_end && unsafe { *bytes.get_unchecked(t_start) }.is_ascii_whitespace() {
                    t_start += 1;
                }
                while t_end > t_start && unsafe { *bytes.get_unchecked(t_end - 1) }.is_ascii_whitespace() {
                    t_end -= 1;
                }
                if t_end > t_start {
                    let text = unsafe { xml.get_unchecked(t_start..t_end) };
                    if on_token(Token::Text(text)).is_break() {
                        return;
                    }
                }
            }
            pos = end_pos;
        }
    }
}

pub fn extract_attributes(xml: &str, mut offset: usize) -> Vec<(&str, &str)> {
    let mut attrs = Vec::new();
    let bytes = xml.as_bytes();
    let len = bytes.len();

    // Skip '<'
    if offset < len && bytes[offset] == b'<' {
        offset += 1;
    } else {
        return attrs;
    }

    // Skip tag name
    while offset < len {
        let b = bytes[offset];
        if b.is_ascii_whitespace() || b == b'>' || b == b'/' {
            break;
        }
        offset += 1;
    }

    loop {
        // Skip whitespace
        while offset < len && bytes[offset].is_ascii_whitespace() {
            offset += 1;
        }

        if offset >= len || bytes[offset] == b'>' || bytes[offset] == b'/' {
            break;
        }

        // Parse key
        let key_start = offset;
        while offset < len {
            let b = bytes[offset];
            if b == b'=' || b.is_ascii_whitespace() || b == b'>' || b == b'/' {
                break;
            }
            offset += 1;
        }
        let key = &xml[key_start..offset];

        // Skip whitespace before '='
        while offset < len && bytes[offset].is_ascii_whitespace() {
            offset += 1;
        }

        if offset < len && bytes[offset] == b'=' {
            offset += 1; // Skip '='

            // Skip whitespace after '='
            while offset < len && bytes[offset].is_ascii_whitespace() {
                offset += 1;
            }

            // Parse value
            if offset < len {
                let quote = bytes[offset];
                if quote == b'"' || quote == b'\'' {
                    offset += 1;
                    let val_start = offset;
                    while offset < len && bytes[offset] != quote {
                        offset += 1;
                    }
                    if offset < len {
                        attrs.push((key, &xml[val_start..offset]));
                        offset += 1; // Skip closing quote
                    }
                } else {
                    // Unquoted value (shouldn't happen in valid XML but handle anyway)
                    let val_start = offset;
                    while offset < len {
                        let b = bytes[offset];
                        if b.is_ascii_whitespace() || b == b'>' || b == b'/' {
                            break;
                        }
                        offset += 1;
                    }
                    attrs.push((key, &xml[val_start..offset]));
                }
            }
        } else {
            // Attribute without value or malformed? Skip it.
             offset += 1;
        }
    }
    attrs
}