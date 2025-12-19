use super::state::TuiState;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};

pub fn handle_input(event: Event, state: &mut TuiState) -> bool {
    if let Event::Key(key_event) = event
        && key_event.kind == KeyEventKind::Press
    {
        return handle_key_press(key_event, state);
    }
    true
}

fn handle_key_press(key_event: KeyEvent, state: &mut TuiState) -> bool {
    match key_event.code {
        KeyCode::Char('q') => return false, // Signal to quit
        KeyCode::Down => state.go_down(),
        KeyCode::Up => state.go_up(),
        KeyCode::Enter | KeyCode::Right => state.enter(),
        KeyCode::Backspace | KeyCode::Left => state.back(),
        KeyCode::PageUp => state.page_up(),
        KeyCode::PageDown => state.page_down(),
        KeyCode::Home => state.home(),
        KeyCode::End => state.end(),
        KeyCode::Char(' ') => state.toggle_info(),
        _ => {}
    }
    true
}
