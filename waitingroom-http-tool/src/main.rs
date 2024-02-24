use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use waitingroom_http_tool::app::{App, AppResult};
use waitingroom_http_tool::event::{Event, EventHandler};
use waitingroom_http_tool::handler::{handle_key_events, handle_mouse_events};
use waitingroom_http_tool::tui::Tui;

#[tokio::main]
async fn main() -> AppResult<()> {
    let (tx_tasks, mut rx_tasks) = tokio::sync::mpsc::channel(100);
    let (tx_updates, rx_updates) = tokio::sync::mpsc::channel(100);

    // Create an application.
    let mut app = App::new(tx_tasks, rx_updates);

    // Start the background task.
    tokio::spawn(async move {
        waitingroom_http_tool::task::background_task(tx_updates, &mut rx_tasks).await;
    });

    // Initialize the terminal user interface.
    let backend = CrosstermBackend::new(io::stderr());
    let terminal = Terminal::new(backend)?;
    let events = EventHandler::new(250);
    let mut tui = Tui::new(terminal, events);
    tui.init()?;

    // Start the main loop.
    while app.running {
        // Render the user interface.
        tui.draw(&mut app)?;
        // Handle events.
        match tui.events.next().await? {
            Event::Tick => app.tick(),
            Event::Key(key_event) => handle_key_events(key_event, &mut app).await?,
            Event::Mouse(mouse_event) => handle_mouse_events(mouse_event, &mut app).await?,
            Event::Resize(_, _) => {}
        }
    }

    // Exit the user interface.
    tui.exit()?;
    Ok(())
}
