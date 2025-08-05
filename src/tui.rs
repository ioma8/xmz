use crate::parser::{stream_xml, Token, Break, Continue};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarState},
};
use std::collections::HashMap;
use std::io::{self, stdout, Stdout};

struct Level {
    tag: Option<String>,
    children: Vec<(String, Option<String>)>, // (tag, text_if_leaf)
}

struct TuiState {
    stack: Vec<Level>,
    selected: usize,
    list_state: ListState,
    children_cache: HashMap<Option<String>, Vec<(String, Option<String>)>>,
    xml: String,
    scrollbar_state: ScrollbarState,
    items_len: usize,
}

fn get_children_cached(
    xml: &str,
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
    let mut last_tag: Option<String> = None;
    let mut last_text: Option<String> = None;
    let mut collecting_text = false;
    stream_xml(xml, |token| {
        match token {
            Token::StartTag(name) => {
                if let Some(parent) = parent_tag {
                    if !inside && name == parent {
                        inside = true;
                        parent_matched = true;
                        return Continue(());
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
                            return Break(());
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
        Continue(())
    });
    cache.insert(key, children.clone());
    children
}

fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

fn get_root_tag(xml: &str) -> Option<String> {
    let mut root_tag = None;
    stream_xml(xml, |token| {
        if let Token::StartTag(name) = token {
            root_tag = Some(name.to_string());
            return Break(());
        }
        Continue(())
    });
    root_tag
}

fn create_main_block(current: &Level) -> Block {
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
    Block::default()
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
        .bg(Color::Rgb(30, 30, 40))
}

fn create_list<'a>(current: &'a Level, block: Block<'a>) -> List<'a> {
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
    List::new(items)
        .block(block)
        .highlight_symbol("→ ")
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .bg(Color::Rgb(40, 40, 60))
                .add_modifier(Modifier::BOLD | Modifier::REVERSED),
        )
        .bg(Color::Rgb(30, 30, 40))
}

fn create_help_paragraph() -> Paragraph<'static> {
    let key_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let help_spans = vec![
        Span::raw("Use "),
        Span::styled("↑/↓", key_style),
        Span::raw(" to move, "),
        Span::styled("Enter/→", key_style),
        Span::raw(" to go in, "),
        Span::styled("Backspace/←", key_style),
        Span::raw(" to go up, "),
        Span::styled("q", key_style),
        Span::raw(" to quit."),
    ];
    let help_line = Line::from(help_spans).alignment(Alignment::Center);

    Paragraph::new(help_line)
        .block(Block::default().borders(Borders::NONE))
}

fn draw_ui(f: &mut Frame, state: &mut TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
        .split(f.size());

    let main_area = chunks[0];
    let help_area = chunks[1];

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(1)].as_ref())
        .split(main_area);

    let list_area = main_chunks[0];
    let scrollbar_area = main_chunks[1];

    let current = state.stack.last().unwrap();
    state.list_state.select(Some(state.selected));
    let block = create_main_block(current);
    let list = create_list(current, block.clone());
    let help = create_help_paragraph();

    let shadow = Block::default()
        .borders(Borders::NONE)
        .bg(Color::Rgb(20, 20, 28));
    let shadow_rect = Rect {
        x: 2,
        y: 2,
        width: main_area.width.saturating_sub(4),
        height: main_area.height.saturating_sub(4),
    };
    f.render_widget(shadow, shadow_rect);
    f.render_stateful_widget(list, list_area, &mut state.list_state);
    f.render_widget(help, help_area);

    state.scrollbar_state = state.scrollbar_state.content_length(state.items_len);

    let scrollbar = Scrollbar::default()
        .orientation(ratatui::widgets::ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));

    f.render_stateful_widget(
        scrollbar,
        scrollbar_area.inner(&Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut state.scrollbar_state,
    );
}

fn handle_key_down(state: &mut TuiState) {
    let current = state.stack.last().unwrap();
    if state.selected + 1 < current.children.len() {
        state.selected += 1;
    }
    state.list_state.select(Some(state.selected));
    state.scrollbar_state = state.scrollbar_state.position(state.selected);
}

fn handle_key_up(state: &mut TuiState) {
    if state.selected > 0 {
        state.selected -= 1;
    }
    state.list_state.select(Some(state.selected));
    state.scrollbar_state = state.scrollbar_state.position(state.selected);
}

fn handle_key_enter(state: &mut TuiState) {
    let current = state.stack.last().unwrap();
    if let Some((tag, _)) = current.children.get(state.selected) {
        let children = get_children_cached(&state.xml, Some(tag), &mut state.children_cache);
        state.items_len = children.len();
        state.stack.push(Level {
            tag: Some(tag.clone()),
            children,
        });
        state.selected = 0;
        state.list_state.select(Some(state.selected));
    }
}

fn handle_key_backspace(state: &mut TuiState) {
    if state.stack.len() > 1 {
        state.stack.pop();
        state.selected = 0;
        state.list_state.select(Some(state.selected));
        state.items_len = state.stack.last().unwrap().children.len();
    }
}

fn handle_input(key_event: KeyEvent, state: &mut TuiState) {
    match key_event.code {
        KeyCode::Char('q') => {
            // This will be handled in the main loop to break
        }
        KeyCode::Down => handle_key_down(state),
        KeyCode::Up => handle_key_up(state),
        KeyCode::Enter | KeyCode::Right => handle_key_enter(state),
        KeyCode::Backspace | KeyCode::Left => handle_key_backspace(state),
        _ => {}
    }
}

pub fn run_tui(xml: &str) -> io::Result<()> {
    let root_tag = get_root_tag(xml);
    let children = match &root_tag {
        Some(tag) => vec![(tag.clone(), None)],
        None => vec![],
    };
    let items_len = children.len();

    let mut state = TuiState {
        stack: vec![Level {
            tag: None,
            children,
        }],
        selected: 0,
        list_state: ListState::default(),
        children_cache: HashMap::new(),
        xml: xml.to_string(),
        scrollbar_state: ScrollbarState::default(),
        items_len,
    };

    let mut terminal = setup_terminal()?;

    loop {
        terminal.draw(|f| draw_ui(f, &mut state))?;

        if event::poll(std::time::Duration::from_millis(200))? {
            if let Event::Key(key_event) = event::read()? {
                if key_event.kind == event::KeyEventKind::Press {
                    if key_event.code == KeyCode::Char('q') {
                        break;
                    }
                    handle_input(key_event, &mut state);
                }
            }
        }
    }

    restore_terminal()
}
