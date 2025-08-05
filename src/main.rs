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

    // Process in larger chunks for better cache performance
    while pos < len {
        // Skip whitespace with SIMD-like approach - check multiple bytes at once
        while pos + 8 <= len {
            // Check 8 bytes at once using u64
            let chunk = unsafe { std::ptr::read_unaligned(bytes.as_ptr().add(pos) as *const u64) };

            // Check if any byte in the chunk is not whitespace
            // Use bitwise operations to check for non-whitespace
            let has_non_whitespace = chunk & 0x8080808080808080 != 0 || // high bit set (non-ASCII)
                (chunk ^ 0x2020202020202020) & 0x7F7F7F7F7F7F7F7F != 0; // not space

            if has_non_whitespace {
                // Fall back to byte-by-byte for the chunk
                break;
            }

            // Check each byte individually if bulk check suggests all whitespace
            let mut found_non_ws = false;
            for i in 0..8 {
                let byte = unsafe { *bytes.get_unchecked(pos + i) };
                if !byte.is_ascii_whitespace() {
                    found_non_ws = true;
                    break;
                }
            }

            if found_non_ws {
                break;
            }

            pos += 8;
        }

        // Handle remaining bytes individually
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

                // Optimized search for '>'
                while end_pos + 8 <= len {
                    let chunk = unsafe {
                        std::ptr::read_unaligned(bytes.as_ptr().add(end_pos) as *const u64)
                    };

                    // Check for '>' character (0x3E) in any of the 8 bytes
                    let gt_mask = chunk ^ 0x3E3E3E3E3E3E3E3E;
                    let has_gt =
                        (gt_mask.wrapping_sub(0x0101010101010101) & !gt_mask & 0x8080808080808080)
                            != 0;

                    if has_gt {
                        // Found '>' in this chunk, find exact position
                        for i in 0..8 {
                            if unsafe { *bytes.get_unchecked(end_pos + i) } == b'>' {
                                end_pos += i;
                                break;
                            }
                        }
                        break;
                    }
                    end_pos += 8;
                }

                // Handle remaining bytes
                while end_pos < len && unsafe { *bytes.get_unchecked(end_pos) } != b'>' {
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
                // Skip comments (<!--...-->) or CDATA (<![CDATA[...]]>) faster
                if pos + 4 < len
                    && unsafe { *bytes.get_unchecked(pos + 2) } == b'-'
                    && unsafe { *bytes.get_unchecked(pos + 3) } == b'-'
                {
                    // Comment: look for -->
                    let mut end_pos = pos + 4;
                    while end_pos + 2 < len {
                        if unsafe { *bytes.get_unchecked(end_pos) } == b'-'
                            && unsafe { *bytes.get_unchecked(end_pos + 1) } == b'-'
                            && unsafe { *bytes.get_unchecked(end_pos + 2) } == b'>'
                        {
                            end_pos += 3;
                            break;
                        }
                        end_pos += 1;
                    }
                    pos = end_pos;
                } else {
                    // Other ! constructs - skip to >
                    let mut end_pos = pos + 2;
                    while end_pos < len && unsafe { *bytes.get_unchecked(end_pos) } != b'>' {
                        end_pos += 1;
                    }
                    pos = if end_pos < len { end_pos + 1 } else { len };
                }
            } else if pos + 1 < len && unsafe { *bytes.get_unchecked(pos + 1) } == b'?' {
                // Skip processing instructions (<?...?>) faster
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
                        if byte <= b' ' || byte == b'/' {
                            // Optimized whitespace check
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
            // Text node - find next '<' with optimized search
            let start = pos;
            let mut end_pos = start;

            // Use SIMD-like approach to find '<' faster
            while end_pos + 8 <= len {
                let chunk =
                    unsafe { std::ptr::read_unaligned(bytes.as_ptr().add(end_pos) as *const u64) };

                // Check for '<' character (0x3C) in any of the 8 bytes
                let lt_mask = chunk ^ 0x3C3C3C3C3C3C3C3C;
                let has_lt =
                    (lt_mask.wrapping_sub(0x0101010101010101) & !lt_mask & 0x8080808080808080) != 0;

                if has_lt {
                    // Found '<' in this chunk, find exact position
                    for i in 0..8 {
                        if unsafe { *bytes.get_unchecked(end_pos + i) } == b'<' {
                            end_pos += i;
                            break;
                        }
                    }
                    break;
                }
                end_pos += 8;
            }

            // Handle remaining bytes
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
    // Pre-allocate with estimated capacity for better performance
    let mut elements_per_level: Vec<usize> = Vec::with_capacity(20);

    let start_time = std::time::Instant::now();

    stream_xml(xml, |token| {
        match token {
            Token::StartTag(_) => {
                // Ensure vector is large enough
                if depth >= elements_per_level.len() {
                    elements_per_level.resize(depth + 1, 0);
                }
                // Count element at current depth before incrementing
                elements_per_level[depth] += 1;

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

    // Print elements per level
    println!("\nElements per depth level:");
    for (level, &count) in elements_per_level.iter().enumerate() {
        if count > 0 {
            let level_name = if level == 0 {
                "Root level".to_string()
            } else {
                format!("Depth {}", level)
            };
            println!("  {}: {} elements", level_name, count);
        }
    }

    Ok(())
}
