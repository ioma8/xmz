use std::io;
use crossterm::event;

mod state;
mod ui;
mod input;
mod terminal;

use state::TuiState;
use terminal::{setup_terminal, restore_terminal};
use ui::draw_ui;
use input::handle_input;

pub fn run_tui(xml: &str) -> io::Result<()> {
    let mut state = TuiState::new(xml);
    let mut terminal = setup_terminal()?;

    loop {
        terminal.draw(|f| draw_ui(f, &mut state))?;

        if event::poll(std::time::Duration::from_millis(200))? {
            if !handle_input(event::read()?, &mut state) {
                break;
            }
        }
    }

    restore_terminal()
}
