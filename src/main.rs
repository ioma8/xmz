use memchr::memchr;
use memmap2::Mmap;
use std::{fs::File, str};

#[derive(Debug)]
enum Token<'a> {
    StartTag(&'a str),
    EndTag(&'a str),
    Text(&'a str),
}

/// Streams tokens from XML without allocations.
/// Calls `on_token` for each parsed token.
fn stream_xml<'a, F>(xml: &'a str, mut on_token: F)
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

fn main() -> std::io::Result<()> {
    let file = File::open("psd7003.xml")?;
    let mmap = unsafe { Mmap::map(&file)? };
    let xml = std::str::from_utf8(&mmap).expect("Invalid UTF-8 XML");

    let mut depth: usize = 0;
    let mut max_depth: usize = 0;
    let mut tag_count = 0;
    // Fixed-size arrays for performance and no dynamic allocation
    const MAX_DEPTH: usize = 32;
    const MAX_UNIQUE_TAGS: usize = 128;
    let mut elements_per_level = [0usize; MAX_DEPTH];
    let mut unique_tags_per_level: [[Option<&str>; MAX_UNIQUE_TAGS]; MAX_DEPTH] =
        [[None; MAX_UNIQUE_TAGS]; MAX_DEPTH];
    let mut unique_tag_counts = [0usize; MAX_DEPTH];

    let start_time = std::time::Instant::now();

    stream_xml(xml, |token| {
        match token {
            Token::StartTag(name) => {
                // Count element at current depth before incrementing
                if depth < MAX_DEPTH {
                    elements_per_level[depth] += 1;
                    // Unique tag logic: linear search, pointer fast path, unsafe for bounds
                    let tags = unsafe { unique_tags_per_level.get_unchecked_mut(depth) };
                    let count = unsafe { unique_tag_counts.get_unchecked_mut(depth) };
                    let mut found = false;
                    let name_ptr = name.as_ptr();
                    let name_len = name.len();
                    for i in 0..*count {
                        if let Some(existing) = unsafe { *tags.get_unchecked(i) } {
                            // Fast path: pointer and length equality
                            if existing.as_ptr() == name_ptr && existing.len() == name_len {
                                found = true;
                                break;
                            }
                            // Fallback: string comparison
                            if existing == name {
                                found = true;
                                break;
                            }
                        }
                    }
                    if !found && *count < MAX_UNIQUE_TAGS {
                        unsafe {
                            *tags.get_unchecked_mut(*count) = Some(name);
                        }
                        *count += 1;
                    }
                }
                depth += 1;
                max_depth = max_depth.max(depth);
                tag_count += 1;
            }
            Token::EndTag(_) => {
                depth = depth.saturating_sub(1); // Prevent underflow
                tag_count += 1;
            }
            Token::Text(_) => {
                // Count text nodes but don't print for performance
            }
        }
    });

    let elapsed = start_time.elapsed();
    println!("Processed {} tags in {:?}", tag_count, elapsed);
    println!("Max depth: {}", max_depth);
    println!("File size: {} bytes", xml.len());
    println!(
        "Processing speed: {:.2} MB/s",
        xml.len() as f64 / elapsed.as_secs_f64() / 1_000_000.0
    );

    // Print elements per level and unique tag names
    println!("\nElements and unique tag names per depth level:");
    for level in 0..MAX_DEPTH {
        let count = elements_per_level[level];
        if count > 0 {
            let level_name = if level == 0 {
                "Root level".to_string()
            } else {
                format!("Depth {}", level)
            };
            println!("  {}: {} elements", level_name, count);
            let tag_count = unique_tag_counts[level];
            if tag_count > 0 {
                // Copy to stack array for sorting
                let mut tag_list: [&str; MAX_UNIQUE_TAGS] = [""; MAX_UNIQUE_TAGS];
                let mut n = 0;
                for i in 0..tag_count {
                    if let Some(tag) = unique_tags_per_level[level][i] {
                        tag_list[n] = tag;
                        n += 1;
                    }
                }
                tag_list[..n].sort_unstable();
                println!("    Unique tags: {}", tag_list[..n].join(", "));
            }
        }
    }

    Ok(())
}
