use memchr::memchr;
use memmap2::Mmap;
use std::env;
use std::{fs::File, str};

// TUI imports
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::io::{self, stdout};

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
    let args: Vec<String> = env::args().collect();
    let use_tui = args.iter().any(|a| a == "--tui");
    if use_tui {
        run_tui()?;
        return Ok(());
    }

    // ...existing code for stats...
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

/// Minimal TUI for XML tree navigation
fn run_tui() -> io::Result<()> {
    // Open and mmap the XML file
    let file = File::open("psd7003.xml")?;
    let mmap = unsafe { Mmap::map(&file)? };
    let xml = std::str::from_utf8(&mmap).expect("Invalid UTF-8 XML");

    // Interactive traversal stack: each entry is (tag_name, children)
    use std::collections::HashMap;
    struct Level {
        tag: Option<String>,
        children: Vec<(String, Option<String>)>, // (tag, text_if_leaf)
    }
    let mut stack: Vec<Level> = Vec::new();
    let mut children_cache: HashMap<Option<String>, Vec<(String, Option<String>)>> = HashMap::new();

    // Helper: get direct children of a tag (or root if tag is None), with cache
    fn get_children_cached<'a>(
        xml: &'a str,
        parent_tag: Option<&str>,
        cache: &mut HashMap<Option<String>, Vec<(String, Option<String>)>>,
    ) -> Vec<(String, Option<String>)> {
        let key = parent_tag.map(|s| s.to_string());
        if let Some(cached) = cache.get(&key) {
            return cached.clone();
        }
        let mut children = Vec::new();
        let mut depth = 0;
        let mut inside = parent_tag.is_none();
        let mut parent_matched = false;
        let mut done = false;
        let mut last_tag: Option<String> = None;
        let mut last_text: Option<String> = None;
        let mut collecting_text = false;
        stream_xml(xml, |token| {
            if done {
                return;
            }
            match token {
                Token::StartTag(name) => {
                    if let Some(parent) = parent_tag {
                        if !inside && name == parent {
                            inside = true;
                            parent_matched = true;
                            return;
                        }
                        if inside {
                            if depth == 0 {
                                // Start possible leaf
                                last_tag = Some(name.to_string());
                                last_text = None;
                                collecting_text = true;
                            }
                            depth += 1;
                        }
                    } else {
                        if depth == 0 {
                            last_tag = Some(name.to_string());
                            last_text = None;
                            collecting_text = true;
                        }
                        depth += 1;
                    }
                }
                Token::EndTag(name) => {
                    if let Some(parent) = parent_tag {
                        if inside {
                            if depth > 0 {
                                depth -= 1;
                            }
                            if depth == 0 && name == parent && parent_matched {
                                done = true;
                                return;
                            }
                            if depth == 0 && collecting_text {
                                if let Some(tag) = last_tag.take() {
                                    children.push((tag, last_text.take()));
                                }
                                collecting_text = false;
                            }
                        }
                    } else {
                        if depth > 0 {
                            depth -= 1;
                        }
                        if depth == 0 && collecting_text {
                            if let Some(tag) = last_tag.take() {
                                children.push((tag, last_text.take()));
                            }
                            collecting_text = false;
                        }
                    }
                }
                Token::Text(txt) => {
                    if collecting_text && depth == 1 {
                        let t = txt.trim();
                        if !t.is_empty() {
                            if let Some(existing) = &mut last_text {
                                existing.push_str(t);
                            } else {
                                last_text = Some(t.to_string());
                            }
                        }
                    }
                }
            }
        });
        cache.insert(key, children.clone());
        children
    }

    // Find the root element (first tag at depth 0)
    let mut root_tag: Option<String> = None;
    stream_xml(xml, |token| {
        if let Token::StartTag(name) = token {
            if root_tag.is_none() {
                root_tag = Some(name.to_string());
            }
        }
    });
    // Start at root: show the root element as the only selectable item
    stack.push(Level {
        tag: None,
        children: match &root_tag {
            Some(tag) => vec![(tag.clone(), None)],
            None => vec![],
        },
    });
    let mut selected = 0usize;
    let mut list_state = ListState::default();

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        let current = stack.last().unwrap();
        list_state.select(Some(selected));
        terminal.draw(|f| {
            let size = f.size();
            let n_children = current.children.len();
            let title = match &current.tag {
                Some(t) => format!(
                    "<{}>  [{} child{}]",
                    t,
                    n_children,
                    if n_children == 1 { "" } else { "ren" }
                ),
                None => format!(
                    "Root element  [{} child{}]",
                    n_children,
                    if n_children == 1 { "" } else { "ren" }
                ),
            };
            let block = Block::default()
                .title(Line::from(vec![
                    Span::styled(
                        " XML Tree Navigator ",
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Blue)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        title,
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue));
            let items: Vec<ListItem> = current
                .children
                .iter()
                .map(|(tag, text)| {
                    if let Some(text) = text {
                        ListItem::new(Line::from(vec![
                            Span::styled(
                                tag,
                                Style::default()
                                    .fg(Color::Magenta)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::raw("  "),
                            Span::styled(text, Style::default().fg(Color::Green)),
                        ]))
                    } else {
                        ListItem::new(Span::styled(tag, Style::default().fg(Color::Magenta)))
                    }
                })
                .collect();
            let list = List::new(items)
                .block(block)
                .highlight_symbol("→ ")
                .highlight_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD | Modifier::REVERSED),
                );
            f.render_stateful_widget(list, size, &mut list_state);
            let help = Paragraph::new(Span::styled(
                "Up/Down: Move, Enter: In, Backspace/Left: Up, q: Quit",
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::ITALIC),
            ))
            .block(Block::default().borders(Borders::NONE));
            f.render_widget(
                help,
                Rect {
                    x: 2,
                    y: size.height - 2,
                    width: size.width - 4,
                    height: 1,
                },
            );
        })?;

        if event::poll(std::time::Duration::from_millis(200))? {
            if let Event::Key(key_event) = event::read()? {
                // Only handle key down/press events
                #[allow(deprecated)]
                let is_press = match key_event.kind {
                    event::KeyEventKind::Press => true,
                    // For crossterm <0.27 compatibility, kind may not exist, so default to true
                    _ => false,
                };
                if is_press {
                    match key_event.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Down => {
                            if selected + 1 < current.children.len() {
                                selected += 1;
                            }
                            list_state.select(Some(selected));
                        }
                        KeyCode::Up => {
                            if selected > 0 {
                                selected -= 1;
                            }
                            list_state.select(Some(selected));
                        }
                        KeyCode::Enter | KeyCode::Right => {
                            if let Some((tag, _)) = current.children.get(selected) {
                                let children =
                                    get_children_cached(xml, Some(tag), &mut children_cache);
                                stack.push(Level {
                                    tag: Some(tag.clone()),
                                    children,
                                });
                                selected = 0;
                                list_state.select(Some(selected));
                            }
                        }
                        KeyCode::Backspace | KeyCode::Left => {
                            if stack.len() > 1 {
                                stack.pop();
                                selected = 0;
                                list_state.select(Some(selected));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}
