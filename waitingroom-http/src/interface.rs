use std::{fs::OpenOptions, net::SocketAddr};

use axum::{
    body::Body, extract::{Request, State}, http::HeaderValue, response::Response, routing::get, Router,
};
use axum_extra::extract::{
    cookie::{Cookie, Key},
    SignedCookieJar,
};
use hyper::StatusCode;
use waitingroom_core::{pass::Pass, ticket::Ticket, NodeId};

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum InterfaceMessageRequest {
    Join,
    CheckInTicket(Ticket),
    Leave(Ticket),
    CheckInPass(Pass),
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum InterfaceMessageResponse {
    Join(Ticket),
    CheckInTicket(usize, Ticket),
    Leave(Pass),
    CheckInPass(Option<Pass>),
    Error(String),
}

#[cached::proc_macro::cached(time = 5)]
fn get_active_servers() -> Vec<NodeId> {
    // We'll have a look at the files in the `.locks` directory
    // and return the NodeIds of the servers that are currently active.

    use std::fs;
    use std::path::Path;

    let lock_dir = Path::new(".locks");
    let mut active_servers = Vec::new();

    if let Ok(entries) = fs::read_dir(lock_dir) {
        for entry in entries {
            let entry = entry.unwrap();

            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            if file_name.ends_with(".lock") {
                let node_id = file_name
                    .trim_start_matches("node-")
                    .trim_end_matches(".lock");

                let file = OpenOptions::new()
                    .write(true)
                    .create_new(false)
                    .open(format!(".locks/node-{}.lock", node_id))
                    .unwrap();

                let lock = file_guard::try_lock(&file, file_guard::Lock::Exclusive, 0, 1);

                match lock {
                    Ok(_) => {
                        // We have the lock, so the server is not active. Release the lock and remove the file
                        drop(lock);
                        fs::remove_file(entry.path()).unwrap();
                        continue;
                    }
                    Err(_) => {
                        // We don't have the lock, so the server is active, cause the server has the lock.
                    }
                }

                let node_id = node_id.parse::<NodeId>().unwrap();
                active_servers.push(node_id);
            }
        }
    }

    active_servers
}

#[derive(Debug)]
enum WaitingRoomStatus {
    NoActiveServers,
    NewTicket,
    TicketRefreshed(usize),
    InvalidTicket,
    NewPass,
    PassRefreshed,
    InvalidPass,
}

impl WaitingRoomStatus {
    fn get_text(&self) -> String {
        match self {
            WaitingRoomStatus::NewTicket => "New ticket! Waiting for refresh...".to_string(),
            WaitingRoomStatus::TicketRefreshed(pos) => {
                format!("You are at queue position {}", pos)
            }
            WaitingRoomStatus::InvalidTicket => {
                "Ticket invalid... Rejoining waiting room...".to_string()
            }
            WaitingRoomStatus::InvalidPass => {
                "Pass invalid... Rejoining waiting room...".to_string()
            }
            WaitingRoomStatus::NewPass => "You left the waiting room! Redirecting...".to_string(),
            WaitingRoomStatus::PassRefreshed => "Pass refreshed! Redirecting...".to_string(),
            WaitingRoomStatus::NoActiveServers => {
                "No active servers... The waiting room is broken!".to_string()
            }
        }
    }
}

fn make_response(
    jar: SignedCookieJar,
    refresh: Option<u64>,
    waiting_room_status: WaitingRoomStatus,
) -> (SignedCookieJar, Response) {
    let body = format!(
        "{}\n\nDEBUG: {:?}",
        waiting_room_status.get_text(),
        jar.iter()
            .map(|c| (c.name().to_string(), c.value().to_string()))
            .collect::<Vec<_>>()
    );
    let mut response = Response::new(Body::from(body));
    if let Some(refresh) = refresh {
        response.headers_mut().insert(
            "Refresh",
            HeaderValue::from_str(&format!("{}", refresh)).unwrap(),
        );
    }

    if let WaitingRoomStatus::TicketRefreshed(pos) = waiting_room_status {
        response.headers_mut().insert(
            "X-WR-Position",
            HeaderValue::from_str(&format!("{}", pos)).unwrap(),
        );
    }

    response.headers_mut().insert(
        "X-WR-Status",
        HeaderValue::from_str(&format!("{:?}", waiting_room_status)).unwrap(),
    );

    match waiting_room_status {
        WaitingRoomStatus::InvalidPass | WaitingRoomStatus::InvalidTicket => {
            let jar = jar.remove("ticket");
            let jar = jar.remove("pass");
            (jar, response)
        }
        _ => (jar, response),
    }
}

async fn make_interface_call(
    interface_req: InterfaceMessageRequest,
    node_id: Option<NodeId>,
    client: reqwest::Client,
) -> Result<InterfaceMessageResponse, ()> {
    let active_nodes = get_active_servers();
    if active_nodes.is_empty() {
        return Err(());
    }
    let node_id = match node_id {
        Some(node_id) => {
            if active_nodes.contains(&node_id) {
                node_id
            } else {
                let rand = rand::random::<usize>() % active_nodes.len();
                active_nodes[rand]
            }
        }
        None => {
            let rand = rand::random::<usize>() % active_nodes.len();
            active_nodes[rand]
        }
    };

    let interface_url = format!("http://localhost:{}/int", node_id);

    Ok(client
        .post(&interface_url)
        .json(&interface_req)
        .send()
        .await
        .unwrap()
        .json::<InterfaceMessageResponse>()
        .await
        .unwrap())
}

async fn handle_waitingroom_request(
    State(client): State<reqwest::Client>,
    req: Request,
) -> Result<(SignedCookieJar, Response), StatusCode> {
    let jar = SignedCookieJar::from_headers(req.headers(), Key::from(&[b'a'; 64]));

    // If the `pass` is set, we'll check if it's valid.
    if let Some(pass) = jar.get("pass") {
        let pass = serde_json::from_str(pass.value());
        let pass: Pass = match pass {
            Ok(pass) => pass,
            Err(_) => return Ok(make_response(jar, None, WaitingRoomStatus::InvalidPass)),
        };

        let request = InterfaceMessageRequest::CheckInPass(pass);
        let response = match make_interface_call(request, Some(pass.node_id), client).await {
            Ok(response) => response,
            Err(_) => return Ok(make_response(jar, None, WaitingRoomStatus::NoActiveServers)),
        };

        match response {
            InterfaceMessageResponse::CheckInPass(Some(pass)) => {
                let demo_serer_response = reqwest::get(format!(
                    "http://localhost:9000{}",
                    req.uri().path_and_query().unwrap().as_str()
                ))
                .await
                .unwrap();

                let demo_server_response = demo_serer_response.text().await.unwrap();

                let body: String = format!(
                    "{}\n\nDEBUG: {:?}",
                    demo_server_response,
                    jar.iter()
                        .map(|c| (c.name().to_string(), c.value().to_string()))
                        .collect::<Vec<_>>()
                );

                let pass = serde_json::to_string(&pass).unwrap();
                let jar = jar.add(Cookie::new("pass", pass));
                let mut response = Response::new(body.into());
                response.headers_mut().insert(
                    "X-WR-Status",
                    HeaderValue::from_str(&format!("{:?}", WaitingRoomStatus::PassRefreshed))
                        .unwrap(),
                );
                return Ok((jar, response));
            }
            InterfaceMessageResponse::CheckInPass(None) => {
                return Ok(make_response(jar, None, WaitingRoomStatus::InvalidPass));
            }
            _ => {
                return Ok(make_response(jar, None, WaitingRoomStatus::InvalidPass));
            }
        }
    }

    // Otherwise, we do the same for tickets.
    if let Some(ticket) = jar.get("ticket") {
        let ticket = serde_json::from_str(ticket.value());
        let ticket: Ticket = match ticket {
            Ok(ticket) => ticket,
            Err(_) => return Ok(make_response(jar, None, WaitingRoomStatus::InvalidTicket)),
        };

        let request = InterfaceMessageRequest::CheckInTicket(ticket);
        let response = match make_interface_call(request, Some(ticket.node_id), client.clone()).await {
            Ok(response) => response,
            Err(_) => return Ok(make_response(jar, None, WaitingRoomStatus::NoActiveServers)),
        };

        match response {
            InterfaceMessageResponse::CheckInTicket(pos, ticket) => {
                if pos == 0 {
                    // We are allowed to leave the queue, so we do that.
                    let request = InterfaceMessageRequest::Leave(ticket);
                    let response = match make_interface_call(request, Some(ticket.node_id), client).await {
                        Ok(response) => response,
                        Err(_) => {
                            return Ok(make_response(jar, None, WaitingRoomStatus::NoActiveServers))
                        }
                    };

                    match response {
                        InterfaceMessageResponse::Leave(pass) => {
                            let pass = serde_json::to_string(&pass).unwrap();
                            let jar = jar.add(Cookie::new("pass", pass));
                            let jar = jar.remove("ticket");
                            return Ok(make_response(jar, Some(1), WaitingRoomStatus::NewPass));
                        }
                        _ => {
                            return Ok(make_response(jar, None, WaitingRoomStatus::InvalidTicket));
                        }
                    }
                }
                let refresh_time = ticket.next_refresh_time; // This is the timestamp in ms, we need the time away from now in seconds.
                let refresh = (refresh_time
                    - std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis())
                    / 1000;
                let ticket = serde_json::to_string(&ticket).unwrap();
                let jar = jar.add(Cookie::new("ticket", ticket));
                return Ok(make_response(
                    jar,
                    Some(refresh as u64),
                    WaitingRoomStatus::TicketRefreshed(pos),
                ));
            }
            _ => {
                return Ok(make_response(jar, None, WaitingRoomStatus::InvalidTicket));
            }
        }
    }

    // We don't have a ticket or a pass, so we'll have the user join the queue.
    let request = InterfaceMessageRequest::Join;

    let response = match make_interface_call(request, None, client).await {
        Ok(response) => response,
        Err(_) => return Ok(make_response(jar, None, WaitingRoomStatus::NoActiveServers)),
    };

    match response {
        InterfaceMessageResponse::Join(ticket) => {
            let refresh_time = ticket.next_refresh_time; // This is the timestamp in ms, we need the time away from now in seconds.
            let refresh = (refresh_time
                - std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis())
                / 1000;
            let ticket = serde_json::to_string(&ticket).unwrap();
            let jar = jar.add(Cookie::new("ticket", ticket));
            Ok(make_response(
                jar,
                Some(refresh as u64),
                WaitingRoomStatus::NewTicket,
            ))
        }
        _ => Ok(make_response(jar, None, WaitingRoomStatus::InvalidTicket)),
    }
}

pub(crate) async fn interface(listening_address: SocketAddr) {
    get_active_servers(); // Clear out any stale locks

    let app = Router::new()
        .route(
            "/active_servers",
            get(|_req: Request| async move {
                let active_servers = get_active_servers();
                axum::Json(active_servers)
            }),
        )
        .fallback(get(handle_waitingroom_request))
        .with_state(reqwest::Client::new());

    let listener = tokio::net::TcpListener::bind(listening_address)
        .await
        .unwrap();
    log::info!(
        "Demo HTTP server listening on http://{}",
        listener.local_addr().unwrap()
    );
    axum::serve(listener, app.into_make_service()).await.unwrap();
}
