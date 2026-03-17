mod app;
mod db;
mod models;
mod storage;
mod ui;

use std::{io, time::Duration};

use anyhow::Result;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::App;

struct CliArgs {
    connection: Option<String>,
    query: Option<String>,
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut connection = None;
    let mut query = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--connection" | "-c" if i + 1 < args.len() => {
                connection = Some(args[i + 1].clone());
                i += 2;
            }
            "--query" | "-q" if i + 1 < args.len() => {
                query = Some(args[i + 1].clone());
                i += 2;
            }
            _ => i += 1,
        }
    }
    CliArgs { connection, query }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, args).await;

    // Always restore the terminal, even on error
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, args: CliArgs) -> Result<()> {
    let mut app = App::new(args.connection.as_deref(), args.query.as_deref())?;

    loop {
        terminal.draw(|f| ui::render(f, &app.screen, app.status.as_deref()))?;

        if event::poll(Duration::from_millis(200))?
            && let Event::Key(key) = event::read()?
                && !app.handle_key(key, terminal).await? {
                    break;
                }
    }

    Ok(())
}
