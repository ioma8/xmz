use memchr::memchr;

#[derive(Debug)]
pub enum Token<'a> {
    StartTag(&'a str),
    EndTag(&'a str),
    Text(&'a str),
}

/// Streams tokens from XML without allocations.
/// Calls `on_token` for each parsed token.
pub fn stream_xml<'a, F>(xml: &'a str, mut on_token: F)
where
    F: FnMut(Token<'a>),
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
            // Check for end tag
            if pos + 1 < len && unsafe { *bytes.get_unchecked(pos + 1) } == b'/' {
                // End tag: </name>
                let start = pos + 2;
                let search = &bytes[start..];
                if let Some(rel) = memchr(b'>', search) {
                    let end_pos = start + rel;
                    let name = unsafe { xml.get_unchecked(start..end_pos) };
                    on_token(Token::EndTag(name));
                    pos = end_pos + 1;
                } else {
                    break;
                }
            }
            // Check for XML comments, CDATA, or processing instructions
            else if pos + 3 < len && unsafe { *bytes.get_unchecked(pos + 1) } == b'!' {
                // TODO: Implement skipping comments, CDATA, etc. if needed
                // For now, just skip to next '>'
                let mut end_pos = pos + 2;
                while end_pos < len && unsafe { *bytes.get_unchecked(end_pos) } != b'>' {
                    end_pos += 1;
                }
                pos = if end_pos < len { end_pos + 1 } else { len };
            }
            // Start tag
            else {
                let start = pos + 1;
                let search = &bytes[start..];
                if let Some(rel) = memchr(b'>', search) {
                    let end_pos = start + rel;
                    // Check if it's self-closing (/>))
                    let is_self_closing =
                        end_pos > start && unsafe { *bytes.get_unchecked(end_pos - 1) } == b'/';
                    // Extract tag name (everything before first space or '/')
                    let mut name_end = start;
                    while name_end < end_pos {
                        let byte = unsafe { *bytes.get_unchecked(name_end) };
                        if byte <= b' ' || byte == b'/' {
                            break;
                        }
                        name_end += 1;
                    }
                    let name = unsafe { xml.get_unchecked(start..name_end) };
                    on_token(Token::StartTag(name));
                    if is_self_closing {
                        on_token(Token::EndTag(name));
                    }
                    pos = end_pos + 1;
                } else {
                    break;
                }
            }
        } else {
            // Text node - find next '<' using memchr
            let start = pos;
            let search = &bytes[start..];
            let end_pos = if let Some(rel) = memchr(b'<', search) {
                start + rel
            } else {
                len
            };
            if end_pos > start {
                // In-place trim without allocation
                let mut t_start = start;
                let mut t_end = end_pos;
                while t_start < t_end
                    && unsafe { *bytes.get_unchecked(t_start) }.is_ascii_whitespace()
                {
                    t_start += 1;
                }
                while t_end > t_start
                    && unsafe { *bytes.get_unchecked(t_end - 1) }.is_ascii_whitespace()
                {
                    t_end -= 1;
                }
                if t_end > t_start {
                    let text = unsafe { xml.get_unchecked(t_start..t_end) };
                    on_token(Token::Text(text));
                }
            }
            pos = end_pos;
        }
        // Add to Cargo.toml:
        // memchr = "2"
    }
}
