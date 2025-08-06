use crate::parser::{stream_xml, Token, Continue};
use crossterm::{
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor, Attribute, SetAttribute},
};
use std::io::stdout;

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
        Continue(())
    });

    let elapsed = start_time.elapsed();
    let mut stdout = stdout();

    execute!(stdout, SetAttribute(Attribute::Bold), Print("--- XML Statistics ---\n"), ResetColor).unwrap();
    execute!(stdout, Print("Processed "), SetForegroundColor(Color::Yellow), Print(tag_count), ResetColor, Print(" tags in "), SetForegroundColor(Color::Green), Print(format!("{:?}\n", elapsed)), ResetColor).unwrap();
    execute!(stdout, Print("Max depth: "), SetForegroundColor(Color::Yellow), Print(max_depth), ResetColor, Print("\n")).unwrap();
    execute!(stdout, Print("File size: "), SetForegroundColor(Color::Yellow), Print(xml.len()), ResetColor, Print(" bytes\n")).unwrap();
    execute!(stdout, Print("Processing speed: "), SetForegroundColor(Color::Green), Print(format!("{:.2} MB/s\n", xml.len() as f64 / elapsed.as_secs_f64() / 1_000_000.0)), ResetColor).unwrap();

    execute!(stdout, Print("\n"), SetAttribute(Attribute::Bold), Print("--- Elements and unique tag names per depth level ---\n"), ResetColor).unwrap();
    for level in 0..MAX_DEPTH {
        let count = elements_per_level[level];
        if count > 0 {
            let level_name = if level == 0 {
                "Root level".to_string()
            } else {
                format!("Depth {}", level)
            };
            execute!(stdout, Print("  "), SetForegroundColor(Color::Cyan), Print(format!("{}: ", level_name)), ResetColor, SetForegroundColor(Color::Yellow), Print(count), ResetColor, Print(" elements\n")).unwrap();
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
                execute!(stdout, Print("    Unique tags: "), SetForegroundColor(Color::Magenta), Print(format!("{}\n", tag_list[..n].join(", "))), ResetColor).unwrap();
            }
        }
    }
}
