/// Application.
pub mod app;

/// Terminal events handler.
pub mod event;

/// Widget renderer.
pub mod ui;

/// Terminal user interface.
pub mod tui;

/// Event handler.
pub mod handler;

/// Background task handler.
pub mod task;

pub type TxTask = tokio::sync::mpsc::Sender<task::Task>;
pub type RxUpdate = tokio::sync::mpsc::Receiver<task::UserStatusUpdate>;
