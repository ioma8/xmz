use super::state::{Level, TuiState};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph, Scrollbar},
};

pub fn draw_ui(f: &mut Frame, state: &mut TuiState) {
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

    state.list_state.select(Some(state.selected));
    let current_level = state.get_current_level().clone();
    let block = create_main_block(&current_level);
    let list = create_list(&current_level, block.clone());
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

fn create_main_block(current: &'_ Level) -> Block<'_> {
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
