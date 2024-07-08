mod demo_server;
mod http_network;
mod waiting_room;
mod interface;

pub(crate) use http_network::HttpNetworkProvider;
pub(crate) use interface::InterfaceMessageRequest;

// impl WaitingRoomStatus {
//     fn get_header_value(&self) -> HeaderValue {
//         HeaderValue::from_str(&format!("{:?}", self)).unwrap()
//     }

//     fn get_text(&self) -> String {
//         match self {
//             WaitingRoomStatus::NewTicket => "New ticket! Refreshing now...".to_string(),
//             WaitingRoomStatus::TicketRefreshed(pos) => {
//                 format!("You are at queue position {}", pos)
//             }
//             WaitingRoomStatus::InvalidTicket => {
//                 "Ticket invalid... Rejoining waiting room...".to_string()
//             }
//             WaitingRoomStatus::InvalidPass => {
//                 "Pass invalid... Rejoining waiting room...".to_string()
//             }
//             WaitingRoomStatus::NewPass => "You left the waiting room! Redirecting...".to_string(),
//             WaitingRoomStatus::PassRefreshed => {
//                 panic!("get_text() should not be called on PassRefreshed")
//             }
//         }
//     }
// }

// /// Utility function to create a response with the appropriate headers.
// fn make_response(
//     jar: SignedCookieJar,
//     refresh: Option<u64>,
//     waiting_room_status: WaitingRoomStatus,
// ) -> (SignedCookieJar, Response) {
//     let mut response = Response::new(Body::from(waiting_room_status.get_text()));
//     if let Some(refresh) = refresh {
//         response.headers_mut().insert(
//             "Refresh",
//             HeaderValue::from_str(&format!("{}", refresh)).unwrap(),
//         );
//     }

//     if let WaitingRoomStatus::TicketRefreshed(pos) = waiting_room_status {
//         response.headers_mut().insert(
//             "X-WR-Position",
//             HeaderValue::from_str(&format!("{}", pos)).unwrap(),
//         );
//     }

//     response.headers_mut().insert(
//         "X-WR-Status",
//         HeaderValue::from_str(&format!("{:?}", waiting_room_status)).unwrap(),
//     );
//     (jar, response)
// }

// async fn handler(
//     State(state): State<AppState>,
//     mut req: Request,
// ) -> Result<(SignedCookieJar, Response), StatusCode> {
//     log::debug!("Request to waiting room");
//     let jar = SignedCookieJar::from_headers(req.headers(), state.key.clone());
//     if let Some(pass) = match jar.get("pass") {
//         Some(cookie) => {
//             log::debug!("Pass cookie found");
//             let pass: Pass = serde_json::from_str(cookie.value()).unwrap();
//             Some(pass)
//         }
//         None => None,
//     } {
//         log::add_fields! {
//             "pass_id" => pass.identifier,
//         }
//         let pass = match state
//             .waitingroom
//             .lock()
//             .unwrap()
//             .validate_and_refresh_pass(pass)
//         {
//             Ok(pass) => pass,
//             Err(err) => {
//                 log::debug!("Pass was invalid: {:?}", err);
//                 return Ok(make_response(
//                     jar.remove("pass"),
//                     Some(3),
//                     WaitingRoomStatus::InvalidPass,
//                 ));
//             }
//         };
//         log::debug!("Pass refreshed");
//         let cookie = Cookie::build(("pass", serde_json::to_string(&pass).unwrap()))
//             .secure(true)
//             .http_only(true);

//         let path = req.uri().path();
//         let path_query = req
//             .uri()
//             .path_and_query()
//             .map(|v| v.as_str())
//             .unwrap_or(path);

//         let uri = format!("http://{}{}", state.settings.proxy_address, path_query);

//         *req.uri_mut() = Uri::try_from(uri).unwrap();

//         let mut response = state
//             .client
//             .request(req)
//             .await
//             .map_err(|_| StatusCode::BAD_REQUEST)?
//             .into_response();

//         response.headers_mut().insert(
//             "X-WR-Status",
//             WaitingRoomStatus::PassRefreshed.get_header_value(),
//         );

//         return Ok((jar.add(cookie), response));
//     };

//     let ticket: Option<Ticket> = match jar.get("ticket") {
//         Some(cookie) => {
//             log::debug!("Ticket cookie found");
//             let ticket = serde_json::from_str(cookie.value()).unwrap();
//             Some(ticket)
//         }
//         None => None,
//     };

//     match ticket {
//         Some(ticket) => {
//             log::add_fields! {
//                 "ticket_id" => ticket.identifier,
//             }
//             let checkin_response = match state.waitingroom.lock().unwrap().check_in(ticket) {
//                 Ok(checkin_response) => checkin_response,
//                 Err(err) => {
//                     log::debug!("Ticket was invalid: {:?}", err);
//                     return Ok(make_response(
//                         jar.remove("ticket"),
//                         Some(3),
//                         WaitingRoomStatus::InvalidTicket,
//                     ));
//                 }
//             };

//             log::debug!("Ticket refreshed");

//             if checkin_response.position_estimate == 0 {
//                 log::debug!("User is at the front of the queue");
//                 let pass = state
//                     .waitingroom
//                     .lock()
//                     .unwrap()
//                     .leave(checkin_response.new_ticket)
//                     .unwrap();

//                 let cookie = Cookie::build(("pass", serde_json::to_string(&pass).unwrap()))
//                     .secure(true)
//                     .http_only(true);
//                 return Ok(make_response(
//                     jar.add(cookie).remove("ticket"),
//                     Some(1),
//                     WaitingRoomStatus::NewPass,
//                 ));
//             } else {
//                 log::debug!("User is at position {}", checkin_response.position_estimate);
//                 let cookie = Cookie::build((
//                     "ticket",
//                     serde_json::to_string(&checkin_response.new_ticket).unwrap(),
//                 ))
//                 .secure(true)
//                 .http_only(true);

//                 let now = std::time::SystemTime::now()
//                     .duration_since(std::time::UNIX_EPOCH)
//                     .unwrap()
//                     .as_millis();
//                 return Ok(make_response(
//                     jar.add(cookie),
//                     Some(
//                         ((checkin_response.new_ticket.next_refresh_time as i128 - now as i128)
//                             / 1000) as u64,
//                     ),
//                     WaitingRoomStatus::TicketRefreshed(checkin_response.position_estimate),
//                 ));
//             }
//         }
//         None => {
//             let ticket = state.waitingroom.lock().unwrap().join().unwrap();
//             log::add_fields! {
//                 "ticket_id" => ticket.identifier,
//             }
//             log::debug!("New ticket issued");
//             return Ok(make_response(
//                 jar.add(
//                     Cookie::build(("ticket", serde_json::to_string(&ticket).unwrap()))
//                         .secure(true)
//                         .http_only(true),
//                 ),
//                 Some(1),
//                 WaitingRoomStatus::NewTicket,
//             ));
//         }
//     }
// }

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    env_logger::init();
    // If the first argument is "demo", start the demo server
    if std::env::args().nth(1) == Some("demo".to_string()) {
        demo_server::demo_server("127.0.0.1:9000".parse().unwrap()).await;
        return Ok(());
    }

    if std::env::args().nth(1) == Some("interface".to_string()) {
        interface::interface("127.0.0.1:8000".parse().unwrap()).await;
        return Ok(());
    }

    // Otherwise, we start the waiting room. We start on the port and node ID, which are the same, specified in the first argument, connecting to the server specified in the third argument.
    assert_eq!(
        std::env::args().len(),
        3,
        "Please provide node ID and server address"
    );

    let node_id = std::env::args().nth(1).unwrap();
    let server = std::env::args().nth(2).unwrap();
    waiting_room::waiting_room(
        format!("127.0.0.1:{}", node_id).parse().unwrap(),
        node_id.parse().unwrap(),
        server.parse().unwrap(),
    )
    .await;

    Ok(())
}
