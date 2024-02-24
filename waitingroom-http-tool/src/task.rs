use tokio::sync::mpsc::{Receiver, Sender};
use waitingroom_core::{get_now_time, ticket::TicketIdentifier, Time};

use reqwest::cookie::Jar;

pub enum Task {
    /// Add a new user to the waiting room.
    AddUser,
    /// Remove a user from the waiting room.
    /// Value is the user's ID
    RemoveUser(i32),
}

#[derive(Debug, strum::EnumIs, strum::Display)]
pub enum Status {
    /// The user has not yet requested the waiting room ticket.
    Starting,
    /// The user has been added to the waiting room.
    /// Value is the queue position.
    Waiting {
        position: usize,
        next_refresh: Time,
    },
    OnSite {
        next_refresh: Time,
    },
    Removed,
}

#[derive(Debug)]
pub struct UserStatusUpdate {
    pub user_id: i32,
    pub identifier_id: Option<TicketIdentifier>,
    pub status: Status,
}

pub struct User {
    pub user_id: i32,
    pub cookie_jar: Jar,
    pub next_refresh: Option<Time>,
}

pub async fn background_task(tx: Sender<UserStatusUpdate>, rx: &mut Receiver<Task>) {
    let mut users: Vec<User> = Vec::new();
    let mut user_idx = 0;
    loop {
        tokio::select! {
            Some(task) = rx.recv() => {
                match task {
                    Task::AddUser => {
                        users.push(User {
                            user_id: user_idx,
                            cookie_jar: Jar::default(),
                            next_refresh: Some(Time::MIN),
                        });
                        let user_status = UserStatusUpdate {
                            user_id: user_idx,
                            identifier_id: None,
                            status: Status::Starting,
                        };

                        user_idx += 1;

                        tx.send(user_status).await.unwrap();
                    }
                    Task::RemoveUser(user_id) => {
                        if let Some(index) = users.iter().position(|user| user.user_id == user_id) {
                            users.remove(index);
                        } else {
                            // The user has probably already been removed.
                            // We can ignore this.
                            continue;
                        }

                        let user_status = UserStatusUpdate {
                            user_id,
                            identifier_id: None,
                            status: Status::Removed,
                        };

                        tx.send(user_status).await.unwrap();
                    }
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                for user in users.iter_mut() {
                    if let Some(next_refresh) = user.next_refresh {
                        if next_refresh <= get_now_time() {
                            user.next_refresh = Some(get_now_time() + 1000);
                            let user_status = UserStatusUpdate {
                                user_id: user.user_id,
                                identifier_id: None,
                                status: Status::OnSite {
                                    next_refresh: user.next_refresh.unwrap(),
                                },
                            };
                            tx.send(user_status).await.unwrap();
                        }
                    }
                }
            }
        }
    }
}
