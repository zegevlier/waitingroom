use crate::app::{App, AppResult};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEventKind};

/// Handles the key events and updates the state of [`App`].
pub async fn handle_key_events(key_event: KeyEvent, app: &mut App) -> AppResult<()> {
    match key_event.code {
        // Exit application on `ESC` or `q`
        KeyCode::Esc | KeyCode::Char('q') => {
            app.quit();
        }
        // Exit application on `Ctrl-C`
        KeyCode::Char('c') | KeyCode::Char('C') => {
            if key_event.modifiers == KeyModifiers::CONTROL {
                app.quit();
            }
        }
        // Counter handlers
        KeyCode::Char('a') => {
            app.add_user().await;
        }
        KeyCode::Char('d') => {
            app.remove_user().await;
        }
        KeyCode::Up => {
            app.previous();
        }
        KeyCode::Down => {
            app.next();
        }
        // Other handlers you could add here.
        _ => {}
    }
    Ok(())
}

pub async fn handle_mouse_events(
    mouse_event: crossterm::event::MouseEvent,
    app: &mut App,
) -> AppResult<()> {
    match mouse_event.kind {
        MouseEventKind::ScrollDown => {
            app.next();
        }
        MouseEventKind::ScrollUp => {
            app.previous();
        }
        _ => {}
    }
    Ok(())
}
