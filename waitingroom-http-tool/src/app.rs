use std::error;

use ratatui::widgets::{ScrollbarState, TableState};
use waitingroom_core::Time;

use crate::ui::ITEM_HEIGHT;

/// Application result type.
pub type AppResult<T> = std::result::Result<T, Box<dyn error::Error>>;

/// Application.
#[derive(Debug)]
pub struct App {
    pub running: bool,
    pub tx_tasks: tokio::sync::mpsc::Sender<crate::task::Task>,
    pub rx_updates: tokio::sync::mpsc::Receiver<crate::task::UserStatusUpdate>,
    pub users: Vec<UserStatus>,
    pub table_state: TableState,
    pub scroll_state: ScrollbarState,
}

#[derive(Debug)]
pub struct UserStatus {
    pub user_id: i32,
    pub status: crate::task::Status,
    pub next_refresh: Option<Time>,
    pub queue_position: Option<usize>,
}

impl App {
    /// Constructs a new instance of [`App`].
    pub fn new(tx_tasks: crate::TxTask, rx_updates: crate::RxUpdate) -> Self {
        Self {
            running: true,
            users: Vec::new(),
            table_state: TableState::default().with_selected(0),
            scroll_state: ScrollbarState::new(0),
            tx_tasks,
            rx_updates,
        }
    }

    /// Handles the tick event of the terminal.
    pub fn tick(&mut self) {
        while let Ok(update) = self.rx_updates.try_recv() {
            if let Some(user) = self.users.iter_mut().find(|u| u.user_id == update.user_id) {
                match update.status {
                    crate::task::Status::Starting => {
                        panic!("Invalid state: User is already in the list, but received a Starting status");
                    }
                    crate::task::Status::Waiting {
                        position,
                        next_refresh,
                    } => {
                        user.status = update.status;
                        user.next_refresh = Some(next_refresh);
                        user.queue_position = Some(position);
                    }
                    crate::task::Status::OnSite { next_refresh } => {
                        user.status = update.status;
                        user.next_refresh = Some(next_refresh);
                    }
                    crate::task::Status::Removed => {
                        if let Some(index) =
                            self.users.iter().position(|u| u.user_id == update.user_id)
                        {
                            self.users.remove(index);
                        }
                    }
                }
            } else if update.status.is_starting() {
                self.users.push(UserStatus {
                    user_id: update.user_id,
                    status: update.status,
                    next_refresh: None,
                    queue_position: None,
                });
            } else {
                dbg!(update);
                dbg!(&self.users);
                panic!("Invalid state: User is not in the list");
            }
        }
        self.scroll_state = self.scroll_state.content_length(self.users.len() * ITEM_HEIGHT);
    }

    /// Set running to false to quit the application.
    pub fn quit(&mut self) {
        self.running = false;
    }

    pub async fn add_user(&mut self) {
        self.tx_tasks
            .send(crate::task::Task::AddUser)
            .await
            .expect("Failed to send AddUser task");
    }

    pub async fn remove_user(&mut self) {
        let user_idx = self.table_state.selected().unwrap_or(0);
        let user = match self.users.get(user_idx) {
            Some(user) => user,
            None => {
                // No user to remove, probably because there are no users.
                return;
            },
        
        };
        self.tx_tasks
            .send(crate::task::Task::RemoveUser(user.user_id))
            .await
            .expect("Failed to send RemoveUser task");
    }

    pub fn next(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if self.users.is_empty() || i >= self.users.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i * ITEM_HEIGHT);
    }

    pub fn previous(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if self.users.is_empty() {
                    0
                } else if i == 0 {
                    self.users.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i * ITEM_HEIGHT);
    }
}
