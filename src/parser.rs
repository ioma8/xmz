use memchr::memchr;
use std::ops::ControlFlow;

#[derive(Debug)]
pub enum Token<'a> {
    StartTag(&'a str),
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
                    if on_token(Token::StartTag(name)).is_break() {
                        return;
                    }
                    if is_self_closing {
                        if on_token(Token::EndTag(name)).is_break() {
                            return;
                        }
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