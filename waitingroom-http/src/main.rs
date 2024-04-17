// This initial implementation was just to get something working as quickly as possible.
// It is pretty bad and doesn't have very clean code. This will be improved in the future.

use std::future::IntoFuture;
use std::sync::{Arc, Mutex};

use axum::http::HeaderValue;

use foundations::cli::{Arg, ArgAction, Cli};
use foundations::telemetry::{init_with_server, log, tracing, TelemetryContext};
use hyper::StatusCode;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;

use settings::HttpServerSettings;
use waitingroom_basic::BasicWaitingRoom;
use waitingroom_core::pass::Pass;
use waitingroom_core::ticket::Ticket;
use waitingroom_core::time::SystemTimeProvider;
use waitingroom_core::WaitingRoomUserTriggered;

use axum::{
    body::Body,
    extract::{Request, State},
    http::uri::Uri,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};

use axum_extra::extract::cookie::{Cookie, Key, SignedCookieJar};

mod demo_server;
mod settings;
mod timers;

type Client = hyper_util::client::legacy::Client<HttpConnector, Body>;

#[derive(Clone)]
struct AppState {
    waitingroom: Arc<Mutex<BasicWaitingRoom<SystemTimeProvider>>>,
    client: Client,
    key: Key,
    settings: HttpServerSettings,
}

#[derive(Debug)]
enum WaitingRoomStatus {
    NewTicket,
    TicketRefreshed(usize),
    InvalidTicket,
    NewPass,
    PassRefreshed,
    InvalidPass,
}

impl WaitingRoomStatus {
    fn get_header_value(&self) -> HeaderValue {
        HeaderValue::from_str(&format!("{:?}", self)).unwrap()
    }

    fn get_text(&self) -> String {
        match self {
            WaitingRoomStatus::NewTicket => "New ticket! Refreshing now...".to_string(),
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
            WaitingRoomStatus::PassRefreshed => {
                panic!("get_text() should not be called on PassRefreshed")
            }
        }
    }
}

/// Utility function to create a response with the appropriate headers.
fn make_response(
    jar: SignedCookieJar,
    refresh: Option<u64>,
    waiting_room_status: WaitingRoomStatus,
) -> (SignedCookieJar, Response) {
    let mut response = Response::new(Body::from(waiting_room_status.get_text()));
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
    (jar, response)
}

#[tracing::span_fn("handler")]
async fn handler(
    State(state): State<AppState>,
    mut req: Request,
) -> Result<(SignedCookieJar, Response), StatusCode> {
    log::debug!("Request to waiting room");
    let jar = SignedCookieJar::from_headers(req.headers(), state.key.clone());
    if let Some(pass) = match jar.get("pass") {
        Some(cookie) => {
            log::debug!("Pass cookie found");
            let pass: Pass = serde_json::from_str(cookie.value()).unwrap();
            Some(pass)
        }
        None => None,
    } {
        log::add_fields! {
            "pass_id" => pass.identifier,
        }
        let pass = match state
            .waitingroom
            .lock()
            .unwrap()
            .validate_and_refresh_pass(pass)
        {
            Ok(pass) => pass,
            Err(err) => {
                log::debug!("Pass was invalid: {:?}", err);
                return Ok(make_response(
                    jar.remove("pass"),
                    Some(3),
                    WaitingRoomStatus::InvalidPass,
                ));
            }
        };
        log::debug!("Pass refreshed");
        let cookie = Cookie::build(("pass", serde_json::to_string(&pass).unwrap()))
            .secure(true)
            .http_only(true);

        let path = req.uri().path();
        let path_query = req
            .uri()
            .path_and_query()
            .map(|v| v.as_str())
            .unwrap_or(path);

        let uri = format!("http://{}{}", state.settings.proxy_address, path_query);

        *req.uri_mut() = Uri::try_from(uri).unwrap();

        let mut response = state
            .client
            .request(req)
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?
            .into_response();

        response.headers_mut().insert(
            "X-WR-Status",
            WaitingRoomStatus::PassRefreshed.get_header_value(),
        );

        return Ok((jar.add(cookie), response));
    };

    let ticket: Option<Ticket> = match jar.get("ticket") {
        Some(cookie) => {
            log::debug!("Ticket cookie found");
            let ticket = serde_json::from_str(cookie.value()).unwrap();
            Some(ticket)
        }
        None => None,
    };

    match ticket {
        Some(ticket) => {
            log::add_fields! {
                "ticket_id" => ticket.identifier,
            }
            let checkin_response = match state.waitingroom.lock().unwrap().check_in(ticket) {
                Ok(checkin_response) => checkin_response,
                Err(err) => {
                    log::debug!("Ticket was invalid: {:?}", err);
                    return Ok(make_response(
                        jar.remove("ticket"),
                        Some(3),
                        WaitingRoomStatus::InvalidTicket,
                    ));
                }
            };

            log::debug!("Ticket refreshed");

            if checkin_response.position_estimate == 0 {
                log::debug!("User is at the front of the queue");
                let pass = state
                    .waitingroom
                    .lock()
                    .unwrap()
                    .leave(checkin_response.new_ticket)
                    .unwrap();

                let cookie = Cookie::build(("pass", serde_json::to_string(&pass).unwrap()))
                    .secure(true)
                    .http_only(true);
                return Ok(make_response(
                    jar.add(cookie).remove("ticket"),
                    Some(1),
                    WaitingRoomStatus::NewPass,
                ));
            } else {
                log::debug!("User is at position {}", checkin_response.position_estimate);
                let cookie = Cookie::build((
                    "ticket",
                    serde_json::to_string(&checkin_response.new_ticket).unwrap(),
                ))
                .secure(true)
                .http_only(true);

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis();
                return Ok(make_response(
                    jar.add(cookie),
                    Some(
                        ((checkin_response.new_ticket.next_refresh_time as i128 - now as i128)
                            / 1000) as u64,
                    ),
                    WaitingRoomStatus::TicketRefreshed(checkin_response.position_estimate),
                ));
            }
        }
        None => {
            let ticket = state.waitingroom.lock().unwrap().join().unwrap();
            log::add_fields! {
                "ticket_id" => ticket.identifier,
            }
            log::debug!("New ticket issued");
            return Ok(make_response(
                jar.add(
                    Cookie::build(("ticket", serde_json::to_string(&ticket).unwrap()))
                        .secure(true)
                        .http_only(true),
                ),
                Some(1),
                WaitingRoomStatus::NewTicket,
            ));
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Obtain service information from Cargo.toml
    let service_info = foundations::service_info!();

    let cli = Cli::<HttpServerSettings>::new(
        &service_info,
        vec![Arg::new("dry-run")
            .long("dry-run")
            .action(ArgAction::SetTrue)
            .help("Validate or generate config without running the server")],
    )?;

    if cli.arg_matches.get_flag("dry-run") {
        return Ok(());
    }

    // Initialize telemetry with the settings obtained from the config.
    let telemetry_server_fut = init_with_server(&service_info, &cli.settings.telemetry, vec![])?;
    if let Some(telemetry_server_addr) = telemetry_server_fut.server_addr() {
        log::info!(
            "Telemetry server is listening on http://{}",
            telemetry_server_addr
        );
    }

    // Only start the demo HTTP server if it is enabled in the config.
    if cli.settings.demo_http_server.enabled {
        tokio::spawn(demo_server::demo_server(
            cli.settings.demo_http_server.listening_address,
        ));
    }

    // The waiting room is in an Arc<Mutex<_>>, because it does not support any concurrency.
    let waitingroom = Arc::new(Mutex::new(BasicWaitingRoom::new(
        cli.settings.waitingroom,
        SystemTimeProvider::new(),
    )));

    let timers = timers::timers(waitingroom.clone(), &cli.settings.timer);

    let client: Client =
        hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
            .build(HttpConnector::new());

    let app = Router::new()
        .fallback(get(|state, req| {
            // Each request gets its own telemetry context.
            TelemetryContext::current()
                .with_forked_log()
                .apply(handler(state, req))
        }))
        .with_state(AppState {
            waitingroom,
            client,
            key: Key::from(&hex::decode(&cli.settings.cookie_secret)?),
            settings: cli.settings.clone(),
        });

    let listener =
        tokio::net::TcpListener::bind(std::net::SocketAddr::from(cli.settings.listening_address))
            .await
            .unwrap();
    log::info!(
        "Waiting room listening on http://{}",
        listener.local_addr().unwrap()
    );

    let web_server = axum::serve(listener, app).into_future();

    tokio::join!(timers, web_server, telemetry_server_fut)
        .2
        .unwrap();
    Ok(())
}
