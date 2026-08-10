mod api;
mod app;
#[cfg(test)]
mod fixtures;
mod layout;
mod model;
mod rules;
mod stream;
mod ui;

use std::error::Error;
use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

type AnyError = Box<dyn Error + Send + Sync>;

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    let server_url = server_url();
    let mut app = app::App::new(server_url)?;
    let mut terminal = TerminalSession::start()?;
    let mut redraw = true;

    while !app.quit {
        if redraw {
            terminal.terminal.draw(|frame| ui::render(frame, &app))?;
            redraw = false;
        }
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    app.handle_key(key).await;
                    redraw = true;
                }
                Event::Resize(_, _) => {
                    terminal.terminal.autoresize()?;
                    redraw = true;
                }
                _ => {}
            }
        }
        redraw |= app.poll_if_due().await;
    }

    Ok(())
}

fn server_url() -> String {
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--server" {
            if let Some(value) = arguments.next() {
                return value;
            }
        }
    }
    std::env::var("MAMAHJONG_SERVER_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_owned())
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    fn start() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout))?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}
