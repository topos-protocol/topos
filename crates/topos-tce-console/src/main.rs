use app::App;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, EventStream},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use std::{error::Error, io};
use tui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};

mod app;
mod client;
mod ui;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let app = App::new();
    terminal.clear()?;
    let res = run_app(&mut terminal, app).await;

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App<'_>) -> io::Result<()> {
    let mut receiver = client::open_client().await;
    let mut input = EventStream::new();
    loop {
        tokio::select! {biased;
            Some(Ok(input)) = input.next() => {
                if let Some(_) = app.handle_input(input) {
                    return Ok(());
                }
            },

            Some(update) = receiver.recv() => {
                app.handle_update(update);
            }
        }

        terminal.draw(|f| ui::ui(f, &mut app))?;
    }
}
