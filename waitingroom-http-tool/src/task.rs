use std::sync::Arc;

use tokio::sync::mpsc::{Receiver, Sender};
use waitingroom_core::{get_now_time, Time};

use reqwest::cookie::Jar;

const URL: &str = "http://127.0.0.1:8051";

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
    InQueue {
        position: Option<usize>,
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
    pub status: Status,
}

#[derive(Debug)]
pub struct UserStatus {
    pub user_id: i32,
    pub status: crate::task::Status,
    pub next_refresh: Option<Time>,
    pub queue_position: Option<usize>,
}

pub struct User {
    pub user_id: i32,
    pub cookie_jar: Arc<Jar>,
    pub next_refresh: Option<Time>,
}

async fn refresh_users(users: &mut [User], tx: &Sender<UserStatusUpdate>) {
    for user in users.iter_mut() {
        if let Some(next_refresh) = user.next_refresh {
            if next_refresh <= get_now_time() {
                let client = reqwest::Client::builder()
                    .cookie_provider(user.cookie_jar.clone())
                    .build()
                    .unwrap();

                let response = client.get(URL).send().await.unwrap();

                let status_header = response.headers().get("x-wr-status").unwrap();
                let status = status_header.to_str().unwrap();

                let status = match status {
                    "NewTicket" => {
                        let refresh_header = response.headers().get("refresh").unwrap();
                        let refresh = refresh_header.to_str().unwrap();
                        let next_refresh = refresh.split(';').next().unwrap();
                        let next_refresh_in: Time = next_refresh.parse::<Time>().unwrap() * 1000;
                        let next_refresh = get_now_time() + next_refresh_in;

                        Status::InQueue {
                            position: None,
                            next_refresh,
                        }
                    },
                    s if s.starts_with("TicketRefreshed(") => {
                        let position = s
                            .trim_start_matches("TicketRefreshed(")
                            .trim_end_matches(')')
                            .parse::<usize>()
                            .unwrap();
                        let refresh_header = response.headers().get("refresh").unwrap();
                        let refresh = refresh_header.to_str().unwrap();
                        let next_refresh = refresh.split(';').next().unwrap();
                        let next_refresh_in: Time = next_refresh.parse::<Time>().unwrap() * 1000;
                        let next_refresh = get_now_time() + next_refresh_in;

                        Status::InQueue {
                            position: Some(position),
                            next_refresh,
                        }
                    },
                    "NewPass" => {
                        let refresh_header = response.headers().get("refresh").unwrap();
                        let refresh = refresh_header.to_str().unwrap();
                        let next_refresh = refresh.split(';').next().unwrap();
                        let next_refresh_in: Time = next_refresh.parse::<Time>().unwrap() * 1000;
                        let next_refresh = get_now_time() + next_refresh_in;

                        Status::OnSite {
                            next_refresh,
                        }
                    },

                    "PassRefreshed" => {
                        let next_refresh = get_now_time() + 1000;

                        Status::OnSite {
                            next_refresh,
                        }
                    },
                    _ => {
                        // We recieved an unknown status.
                        // Let's just remove the user.
                        Status::Removed
                    },
                };

                let user_status = UserStatusUpdate {
                    user_id: user.user_id,
                    status,
                };

                tx.send(user_status).await.unwrap();
            }
        }
    }
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
                            cookie_jar: Arc::new(Jar::default()),
                            next_refresh: Some(Time::MIN),
                        });
                        let user_status = UserStatusUpdate {
                            user_id: user_idx,
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
                            status: Status::Removed,
                        };

                        tx.send(user_status).await.unwrap();
                    }
                }
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                refresh_users(&mut users, &tx).await;
            }
        }
    }
}
