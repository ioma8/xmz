use memmap2::Mmap;
use std::{collections::HashMap, fs::File, str};

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

    // Use unsafe indexing for better performance, but keep bounds checks
    while pos < len {
        // Skip whitespace between tokens - optimized loop
        while pos < len {
            let byte = unsafe { *bytes.get_unchecked(pos) };
            if !byte.is_ascii_whitespace() {
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
                let mut end_pos = start;

                // Find closing '>' with direct indexing
                while end_pos < len {
                    if unsafe { *bytes.get_unchecked(end_pos) } == b'>' {
                        break;
                    }
                    end_pos += 1;
                }

                if end_pos < len {
                    let name = unsafe { xml.get_unchecked(start..end_pos) };
                    on_token(Token::EndTag(name));
                    pos = end_pos + 1;
                } else {
                    break;
                }
            }
            // Check for XML comments, CDATA, or processing instructions
            else if pos + 3 < len && unsafe { *bytes.get_unchecked(pos + 1) } == b'!' {
                // Skip comments (<!--...-->) or CDATA (<![CDATA[...]]>)
                let mut end_pos = pos + 2;
                while end_pos < len {
                    if unsafe { *bytes.get_unchecked(end_pos) } == b'>' {
                        break;
                    }
                    end_pos += 1;
                }
                pos = if end_pos < len { end_pos + 1 } else { len };
            } else if pos + 1 < len && unsafe { *bytes.get_unchecked(pos + 1) } == b'?' {
                // Skip processing instructions (<?...?>)
                let mut end_pos = pos + 2;
                while end_pos + 1 < len {
                    if unsafe { *bytes.get_unchecked(end_pos) } == b'?'
                        && unsafe { *bytes.get_unchecked(end_pos + 1) } == b'>'
                    {
                        end_pos += 2;
                        break;
                    }
                    end_pos += 1;
                }
                pos = end_pos;
            }
            // Start tag
            else {
                let start = pos + 1;
                let mut end_pos = start;
                let mut is_self_closing = false;

                // Find closing '>' and check for self-closing
                while end_pos < len {
                    let byte = unsafe { *bytes.get_unchecked(end_pos) };
                    if byte == b'>' {
                        // Check if it's self-closing (/>)
                        if end_pos > start && unsafe { *bytes.get_unchecked(end_pos - 1) } == b'/' {
                            is_self_closing = true;
                        }
                        break;
                    }
                    end_pos += 1;
                }

                if end_pos < len {
                    // Extract tag name (everything before first space or '/')
                    let mut name_end = start;
                    while name_end < end_pos {
                        let byte = unsafe { *bytes.get_unchecked(name_end) };
                        if byte == b' '
                            || byte == b'\t'
                            || byte == b'\n'
                            || byte == b'\r'
                            || byte == b'/'
                        {
                            break;
                        }
                        name_end += 1;
                    }

                    let name = unsafe { xml.get_unchecked(start..name_end) };
                    on_token(Token::StartTag(name));

                    // If self-closing, also emit end tag
                    if is_self_closing {
                        on_token(Token::EndTag(name));
                    }

                    pos = end_pos + 1;
                } else {
                    break;
                }
            }
        } else {
            // Text node - find next '<' with direct indexing
            let start = pos;
            let mut end_pos = start;

            while end_pos < len && unsafe { *bytes.get_unchecked(end_pos) } != b'<' {
                end_pos += 1;
            }

            if end_pos > start {
                let text = unsafe { xml.get_unchecked(start..end_pos) };
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    on_token(Token::Text(trimmed));
                }
            }
            pos = end_pos;
        }
    }
}

fn main() -> std::io::Result<()> {
    let file = File::open("psd7003.xml")?;
    let mmap = unsafe { Mmap::map(&file)? };
    let xml = std::str::from_utf8(&mmap).expect("Invalid UTF-8 XML");

    let mut depth: usize = 0;
    let mut max_depth: usize = 0;
    let mut tag_count = 0;
    let mut elements_per_level: HashMap<usize, usize> = HashMap::new();

    let start_time = std::time::Instant::now();

    stream_xml(xml, |token| {
        match token {
            Token::StartTag(name) => {
                // Count element at current depth before incrementing
                *elements_per_level.entry(depth).or_insert(0) += 1;

                depth += 1;
                max_depth = max_depth.max(depth);
                tag_count += 1;

                // Debug: print first few tags to verify parsing
                if tag_count <= 10 {
                    println!("Start: {} (depth: {})", name, depth - 1);
                }
            }
            Token::EndTag(name) => {
                if tag_count <= 10 {
                    println!("End: {} (depth: {})", name, depth - 1);
                }
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

    // Print elements per level
    println!("\nElements per depth level:");
    let mut levels: Vec<_> = elements_per_level.iter().collect();
    levels.sort_by_key(|(level, _)| *level);

    for (level, count) in levels {
        let level_name = if *level == 0 {
            "Root level".to_string()
        } else {
            format!("Depth {}", level)
        };
        println!("  {}: {} elements", level_name, count);
    }

    Ok(())
}
