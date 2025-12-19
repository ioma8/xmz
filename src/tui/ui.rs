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

    // Ensure selected index is within the valid range before applying it to the list state.
    let items_len = state.items_len;
    if items_len == 0 {
        state.selected = 0;
    } else if state.selected >= items_len {
        state.selected = items_len.saturating_sub(1);
    }
    state.list_state.select(Some(state.selected));
    
    // Extract data from level without holding borrow across the mutable operations
    let current_level = state.get_current_level();
    let block = create_main_block(current_level, state.selected);
    let list = create_list(current_level, block, state.selected);
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

    if state.show_info_popup {
        if let Some((ref attrs, child_count)) = state.info_popup_data {
            let area = centered_rect(60, 50, f.size());
            f.render_widget(ratatui::widgets::Clear, area);

            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Children count: ", Style::default().fg(Color::Cyan)),
                    Span::styled(child_count.to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(""),
                Line::from(Span::styled("Attributes:", Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED))),
            ];

            if attrs.is_empty() {
                lines.push(Line::from(Span::styled("  (none)", Style::default().fg(Color::DarkGray))));
            } else {
                for (key, val) in attrs {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(*key, Style::default().fg(Color::Magenta)),
                        Span::raw(" = "),
                        Span::styled(*val, Style::default().fg(Color::Green)),
                    ]));
                }
            }

            let block = Block::default()
                .title(" Element Details ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White))
                .bg(Color::Rgb(40, 40, 50));
            
            let paragraph = Paragraph::new(lines)
                .block(block)
                .wrap(ratatui::widgets::Wrap { trim: true });
            
            f.render_widget(paragraph, area);
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn create_main_block<'a>(current: &Level<'a>, selected_index: usize) -> Block<'a> {
    let n_children = current.children.len();
    let current_pos = if n_children > 0 { selected_index + 1 } else { 0 };
    
    let title = match &current.tag {
        Some(t) => format!(
            "<{}>  [{}/{}]",
            t,
            current_pos,
            n_children
        ),
        None => format!(
            "Root element  [{}/{}]",
            current_pos,
            n_children
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

fn create_list<'a>(current: &Level<'a>, block: Block<'a>, selected_index: usize) -> List<'a> {
    let items: Vec<ListItem> = current
        .children
        .iter()
        .enumerate()
        .map(|(i, (tag, text, _, attrs_raw))| {
            let mut spans = vec![
                Span::styled(
                    *tag,
                    Style::default()
                        .fg(Color::Rgb(255, 180, 255))
                        .add_modifier(Modifier::BOLD),
                ),
            ];

            let trimmed_attrs = attrs_raw.replace('\n', " ");
            let trimmed_attrs = trimmed_attrs.trim();
            if !trimmed_attrs.is_empty() {
                let display = if trimmed_attrs.len() > 40 {
                    format!(" {}...", &trimmed_attrs[..40])
                } else {
                    format!(" {}", trimmed_attrs)
                };
                
                let attr_color = if i == selected_index {
                    Color::LightCyan 
                } else {
                    Color::DarkGray
                };

                spans.push(Span::styled(
                    display,
                    Style::default().fg(attr_color),
                ));
            }

            if let Some(text) = text {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    *text,
                    Style::default()
                        .fg(Color::Rgb(120, 255, 120))
                        .add_modifier(Modifier::ITALIC),
                ));
            }

            ListItem::new(Line::from(spans))
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
        Span::styled("Space", key_style),
        Span::raw(" to show details, "),
        Span::styled("q", key_style),
        Span::raw(" to quit."),
    ];
    let help_line = Line::from(help_spans).alignment(Alignment::Center);

    Paragraph::new(help_line)
        .block(Block::default().borders(Borders::NONE))
}
