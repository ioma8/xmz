use crate::parser::{stream_xml, Token};

const MAX_DEPTH: usize = 32;
const MAX_UNIQUE_TAGS: usize = 128;

pub fn print_stats(xml: &str) {
    let mut depth: usize = 0;
    let mut max_depth: usize = 0;
    let mut tag_count = 0;
    let mut elements_per_level = [0usize; MAX_DEPTH];
    let mut unique_tags_per_level: [[Option<&str>; MAX_UNIQUE_TAGS]; MAX_DEPTH] = [[None; MAX_UNIQUE_TAGS]; MAX_DEPTH];
    let mut unique_tag_counts = [0usize; MAX_DEPTH];

    let start_time = std::time::Instant::now();

    stream_xml(xml, |token| {
        match token {
            Token::StartTag(name) => {
                if depth < MAX_DEPTH {
                    elements_per_level[depth] += 1;
                    let tags = unsafe { unique_tags_per_level.get_unchecked_mut(depth) };
                    let count = unsafe { unique_tag_counts.get_unchecked_mut(depth) };
                    let mut found = false;
                    let name_ptr = name.as_ptr();
                    let name_len = name.len();
                    for i in 0..*count {
                        if let Some(existing) = unsafe { *tags.get_unchecked(i) } {
                            if existing.as_ptr() == name_ptr && existing.len() == name_len {
                                found = true;
                                break;
                            }
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
                depth = depth.saturating_sub(1);
                tag_count += 1;
            }
            Token::Text(_) => {}
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
}
