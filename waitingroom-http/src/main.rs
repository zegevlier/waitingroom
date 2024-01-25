// This initial implementation was just to get something working as quickly as possible.
// It is pretty bad and doesn't have very clean code. This will be improved in the future.

use std::future::IntoFuture;
use std::sync::{Arc, Mutex};

use axum::http::HeaderValue;

use foundations::cli::{Arg, ArgAction, Cli};
use foundations::telemetry::{init_with_server, log};
use hyper::StatusCode;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;

use settings::HttpServerSettings;
use tokio::time::{self, Duration};
use waitingroom_basic::BasicWaitingRoom;
use waitingroom_core::pass::Pass;
use waitingroom_core::ticket::Ticket;
use waitingroom_core::{WaitingRoomTimerTriggered, WaitingRoomUserTriggered};

use axum::{
    body::Body,
    extract::{Request, State},
    http::uri::Uri,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};

use axum_extra::extract::cookie::{Cookie, Key, SignedCookieJar};

mod settings;

type Client = hyper_util::client::legacy::Client<HttpConnector, Body>;

const CLEANUP_INTERVAL: u64 = 10 * 1000;
const ENSURE_CORRECT_COUNT_INTERVAL: u64 = 10 * 1000;
const SYNC_USER_COUNTS_INTERVAL: u64 = 10 * 1000;

#[derive(Clone)]
struct AppState {
    waiting_room: Arc<Mutex<BasicWaitingRoom>>,
    client: Client,
    key: Key,
}

async fn server() {
    let app = Router::new().route("/", get(|| async { "Hello, world!" }));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn handler(
    State(state): State<AppState>,
    mut req: Request,
) -> Result<(SignedCookieJar, Response), StatusCode> {
    let jar = SignedCookieJar::from_headers(req.headers(), state.key.clone());
    if let Some(pass) = match jar.get("pass") {
        Some(cookie) => {
            let pass: Pass = serde_json::from_str(cookie.value()).unwrap();
            Some(pass)
        }
        None => None,
    } {
        let pass = match state
            .waiting_room
            .lock()
            .unwrap()
            .validate_and_refresh_pass(pass)
        {
            Ok(pass) => pass,
            Err(_) => return Ok((jar, Response::new(Body::from("Pass invalid")))),
        };
        let cookie = Cookie::build(("pass", serde_json::to_string(&pass).unwrap()))
            .secure(true)
            .http_only(true);

        let path = req.uri().path();
        let path_query = req
            .uri()
            .path_and_query()
            .map(|v| v.as_str())
            .unwrap_or(path);

        let uri = format!("http://127.0.0.1:3000{}", path_query);

        *req.uri_mut() = Uri::try_from(uri).unwrap();

        let mut response = state
            .client
            .request(req)
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?
            .into_response();

        response
            .headers_mut()
            .insert("X-WaitingRoom-Type", HeaderValue::from_static("Basic"));

        return Ok((jar.add(cookie), response));
    };

    let ticket: Option<Ticket> = match jar.get("ticket") {
        Some(cookie) => {
            let ticket = serde_json::from_str(cookie.value()).unwrap();
            Some(ticket)
        }
        None => None,
    };

    let (ticket, text) = match ticket {
        Some(ticket) => {
            let checkin_reponse = match state.waiting_room.lock().unwrap().check_in(ticket) {
                Ok(checkin_reponse) => checkin_reponse,
                Err(err) => {
                    return Ok((
                        jar,
                        Response::new(Body::from(format!("Ticket invalid: {:?}", err))),
                    ))
                }
            };
            if checkin_reponse.position_estimate == 0 {
                let pass = state
                    .waiting_room
                    .lock()
                    .unwrap()
                    .leave(checkin_reponse.new_ticket)
                    .unwrap();
                let cookie = Cookie::build(("pass", serde_json::to_string(&pass).unwrap()));
                let mut response = Response::new(Body::from(
                    "You have left the queue! Redirecting...".to_string(),
                ));
                response
                    .headers_mut()
                    .insert("Refresh", HeaderValue::from_static("0"));
                return Ok((jar.add(cookie), response));
            } else {
                (
                    checkin_reponse.new_ticket,
                    format!(
                        "You are at queue poisiton {}",
                        checkin_reponse.position_estimate
                    ),
                )
            }
        }
        None => {
            let ticket = state.waiting_room.lock().unwrap().join().unwrap();
            (ticket, "New ticket".to_string())
        }
    };

    let cookie = Cookie::build(("ticket", serde_json::to_string(&ticket).unwrap()))
        .secure(true)
        .http_only(true);
    let mut response = Response::new(Body::from(text));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    response.headers_mut().insert(
        "Refresh",
        HeaderValue::from_str(&format!(
            "{}",
            ((ticket.next_refresh_time as i128 - now as i128) / 1000)
        ))
        .unwrap(),
    );

    Ok((jar.add(cookie), response))

    // Ok((jar, response))
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

    // Initialize telemetry with the settings obtained from the config. Don't drive the telemetry
    // server yet - we have some extra security-related steps to do.
    let tele_serv_fut = init_with_server(&service_info, &cli.settings.telemetry, vec![])?;
    if let Some(tele_serv_addr) = tele_serv_fut.server_addr() {
        log::info!("Telemetry server is listening on http://{}", tele_serv_addr);
    }

    let waiting_room = Arc::new(Mutex::new(BasicWaitingRoom::new()));

    let mut cleanup_interval = time::interval(Duration::from_millis(CLEANUP_INTERVAL));
    let mut ensure_correct_count_interval =
        time::interval(Duration::from_millis(ENSURE_CORRECT_COUNT_INTERVAL));
    let mut sync_user_counts_interval =
        time::interval(Duration::from_millis(SYNC_USER_COUNTS_INTERVAL));

    let waiting_room_clone = waiting_room.clone();
    let cleanup = async move {
        loop {
            cleanup_interval.tick().await;
            let mut waiting_room = waiting_room_clone.lock().unwrap();
            waiting_room.cleanup().unwrap();
        }
    };

    let waiting_room_clone = waiting_room.clone();

    let ensure_correct_count = async move {
        loop {
            ensure_correct_count_interval.tick().await;
            let mut waiting_room = waiting_room_clone.lock().unwrap();
            waiting_room.ensure_correct_user_count().unwrap();
        }
    };

    let waiting_room_clone = waiting_room.clone();

    let sync_user_counts = async move {
        loop {
            sync_user_counts_interval.tick().await;
            let mut waiting_room = waiting_room_clone.lock().unwrap();
            waiting_room.sync_user_counts().unwrap();
        }
    };

    tokio::spawn(server());

    let client: Client =
        hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
            .build(HttpConnector::new());

    let app = Router::new().route("/", get(handler)).with_state(AppState {
        waiting_room,
        client,
        key: Key::generate(),
    });

    let listener = tokio::net::TcpListener::bind("127.0.0.1:4000")
        .await
        .unwrap();

    let webserver = axum::serve(listener, app).into_future();

    tokio::join!(
        webserver,
        cleanup,
        ensure_correct_count,
        sync_user_counts,
        tele_serv_fut
    )
    .0
    .unwrap();
    Ok(())
}
