use crate::parser::{stream_xml, Token};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use memmap2::Mmap;
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, stdout};

struct Level {
    tag: Option<String>,
    children: Vec<(String, Option<String>)>, // (tag, text_if_leaf)
}

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

pub fn run_tui() -> io::Result<()> {
    let file = File::open("psd7003.xml")?;
    let mmap = unsafe { Mmap::map(&file)? };
    let xml = std::str::from_utf8(&mmap).expect("Invalid UTF-8 XML");

    let mut stack: Vec<Level> = Vec::new();
    let mut children_cache: HashMap<Option<String>, Vec<(String, Option<String>)>> = HashMap::new();

    let mut root_tag: Option<String> = None;
    stream_xml(xml, |token| {
        if let Token::StartTag(name) = token {
            if root_tag.is_none() {
                root_tag = Some(name.to_string());
            }
        }
    });

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
                    "<{}>  [{} child{}]",
                    t,
                    n_children,
                    if n_children == 1 { "" } else { "ren" }
                ),
                None => format!(
                    "Root element  [{} child{}]",
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
                .border_style(Style::default().fg(Color::Gray))
                .bg(Color::Rgb(30, 30, 40));

            let shadow = Block::default()
                .borders(Borders::NONE)
                .bg(Color::Rgb(20, 20, 28));
            let shadow_rect = Rect {
                x: 2,
                y: 2,
                width: size.width.saturating_sub(4),
                height: size.height.saturating_sub(4),
            };
            f.render_widget(shadow, shadow_rect);

            let items: Vec<ListItem> = current
                .children
                .iter()
                .map(|(tag, text)| {
                    if let Some(text) = text {
                        ListItem::new(Line::from(vec![
                            Span::styled(
                                tag,
                                Style::default()
                                    .fg(Color::Rgb(255, 180, 255))
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::raw("  "),
                            Span::styled(
                                text,
                                Style::default()
                                    .fg(Color::Rgb(120, 255, 120))
                                    .add_modifier(Modifier::ITALIC),
                            ),
                        ]))
                    } else {
                        ListItem::new(Span::styled(
                            tag,
                            Style::default()
                                .fg(Color::Rgb(200, 200, 255))
                                .add_modifier(Modifier::BOLD),
                        ))
                    }
                })
                .collect();
            let list = List::new(items)
                .block(block)
                .highlight_symbol("→ ")
                .highlight_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .bg(Color::Rgb(40, 40, 60))
                        .add_modifier(Modifier::BOLD | Modifier::REVERSED),
                )
                .bg(Color::Rgb(30, 30, 40));
            f.render_stateful_widget(list, size, &mut list_state);
            let help = Paragraph::new(Span::styled(
                "Up/Down: Move, Enter/Right: In, Backspace/Left: Up, q: Quit",
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
                #[allow(deprecated)]
                let is_press = match key_event.kind {
                    event::KeyEventKind::Press => true,
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

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}
