use std::{fs::OpenOptions, future::IntoFuture, io::Write, net::SocketAddr};

use axum::{
    extract::{self},
    routing::{get, post},
    Json, Router,
};
use waitingroom_core::{
    network::Message, settings::GeneralWaitingRoomSettings, NodeId, WaitingRoomError,
    WaitingRoomMessageTriggered, WaitingRoomTimerTriggered,
};

use std::sync::{Arc, Mutex};

use crate::{interface::InterfaceMessageResponse, HttpNetworkProvider, InterfaceMessageRequest};
use waitingroom_core::{
    random::TrueRandomProvider, time::SystemTimeProvider, WaitingRoomUserTriggered,
};
use waitingroom_distributed::{messages::NodeToNodeMessage, DistributedWaitingRoom};

type WR =
    Arc<Mutex<DistributedWaitingRoom<SystemTimeProvider, TrueRandomProvider, HttpNetworkProvider>>>;

#[derive(Clone)]
struct AppState {
    waitingroom: WR,
    network_handle: Arc<Mutex<HttpNetworkProvider>>,
}

#[axum::debug_handler]
async fn handle_incoming_message(
    extract::State(state): extract::State<AppState>,
    extract::Json(payload): extract::Json<Message<NodeToNodeMessage>>,
) -> String {
    log::info!("Received message: {:?}", payload);
    let mut network_handle = state.network_handle.lock().unwrap();
    network_handle.add_message(payload);
    "OK".to_string()
}

#[axum::debug_handler]
async fn handle_incoming_interface(
    extract::State(state): extract::State<AppState>,
    extract::Json(payload): extract::Json<InterfaceMessageRequest>,
) -> Json<InterfaceMessageResponse> {
    log::info!("Received message: {:?}", payload);
    let mut waitingroom = state.waitingroom.lock().unwrap();
    match payload {
        InterfaceMessageRequest::Join => {
            let ticket = match waitingroom.join() {
                Ok(t) => t,
                Err(err) => {
                    return Json(InterfaceMessageResponse::Error(format!(
                        "Error joining waiting room: {:?}",
                        err,
                    )));
                }
            };
            Json(InterfaceMessageResponse::Join(ticket))
        }
        InterfaceMessageRequest::CheckInTicket(t) => {
            let resp = match waitingroom.check_in(t) {
                Ok(r) => r,
                Err(err) => {
                    return Json(InterfaceMessageResponse::Error(format!(
                        "Error checking in ticket: {:?}",
                        err,
                    )));
                }
            };
            Json(InterfaceMessageResponse::CheckInTicket(
                resp.position_estimate,
                resp.new_ticket,
            ))
        }
        InterfaceMessageRequest::Leave(t) => {
            let pass = match waitingroom.leave(t) {
                Ok(p) => p,
                Err(err) => {
                    return Json(InterfaceMessageResponse::Error(format!(
                        "Error leaving waiting room: {:?}",
                        err,
                    )));
                }
            };
            Json(InterfaceMessageResponse::Leave(pass))
        }
        InterfaceMessageRequest::CheckInPass(p) => {
            let pass = match waitingroom.validate_and_refresh_pass(p) {
                Ok(p) => p,
                Err(err) => match err {
                    WaitingRoomError::PassExpired => {
                        return Json(InterfaceMessageResponse::CheckInPass(None))
                    }
                    err => {
                        return Json(InterfaceMessageResponse::Error(format!(
                            "Error checking in pass: {:?}",
                            err,
                        )))
                    }
                },
            };
            Json(InterfaceMessageResponse::CheckInPass(Some(pass)))
        }
    }
}

pub(crate) async fn waiting_room(
    listening_address: SocketAddr,
    node_id: NodeId,
    connecting_node: NodeId,
) {
    log::info!("Starting waiting room with node id {}", node_id);
    // We'll first see if we can take the lock on our node ID's file, if not we can't start the waiting room.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(format!(".locks/node-{}.lock", node_id))
        .unwrap();

    // We'll write our PID to the file, so we can check if the process is still running.
    write!(file, "{}", std::process::id()).unwrap();

    let lock = file_guard::try_lock(&file, file_guard::Lock::Exclusive, 0, 1).unwrap();

    let settings = GeneralWaitingRoomSettings {
        target_user_count: usize::MAX,

        ticket_refresh_time: 5000,
        ticket_expiry_time: 25 * 1000,
        pass_expiry_time: 2 * 1000,

        fault_detection_period: 1000,
        fault_detection_timeout: 300,
        fault_detection_interval: 100,

        eviction_interval: 5000,
        cleanup_interval: 10000,
    }; // TODO: Load from config

    let network_provider = HttpNetworkProvider::new(node_id);

    let waiting_room = DistributedWaitingRoom::new(
        settings,
        node_id,
        SystemTimeProvider::new(),
        TrueRandomProvider::new(),
        network_provider.clone(),
    );

    let app_state = AppState {
        waitingroom: Arc::new(Mutex::new(waiting_room)),
        network_handle: Arc::new(Mutex::new(network_provider)),
    };

    let app = Router::new()
        .route("/msg", post(handle_incoming_message))
        .route("/int", post(handle_incoming_interface))
        .route(
            "/debug",
            get(
                |extract::State(state): extract::State<AppState>| async move {
                    let waitingroom = state.waitingroom.lock().unwrap();
                    format!("{:#?}", waitingroom)
                },
            ),
        )
        .with_state(app_state.clone());

    let listener = tokio::net::TcpListener::bind(listening_address)
        .await
        .unwrap();

    log::info!(
        "Waiting room listening on http://{}",
        listener.local_addr().unwrap()
    );

    let serving = axum::serve(listener, app).into_future();
    let timers = do_timers(app_state.waitingroom.clone());

    app_state
        .waitingroom
        .clone()
        .lock()
        .unwrap()
        .join_at(connecting_node)
        .unwrap();

    tokio::select! {
        _ = serving => {}
        _ = timers => {}
    }

    drop(lock);
    std::fs::remove_file(format!(".locks/node-{}.lock", node_id)).unwrap();
}

async fn do_timers(waitingroom: WR) {
    macro_rules! timer {
        ($name:ident, $interval:expr, $callback:expr) => {
            let mut $name =
                tokio::time::interval(tokio::time::Duration::from_millis($interval as u64));
            let waitingroom_clone = waitingroom.clone();
            let $name = async move {
                log::debug!("Starting timer");
                loop {
                    $name.tick().await;
                    let mut waitingroom = waitingroom_clone.lock().unwrap();
                    match $callback(&mut waitingroom) {
                        Ok(_) => {}
                        Err(err) => {
                            log::error!("Error in timer {}: {:?}", stringify!($name), err);
                        }
                    }
                }
            };
        };
        () => {};
    }

    timer!(cleanup, 1000, DistributedWaitingRoom::cleanup);

    timer!(receive_message, 1, DistributedWaitingRoom::receive_message);

    timer!(
        fault_detection,
        100000000000000000,
        DistributedWaitingRoom::fault_detection
    );

    // Now, we also have evictions. These need to happen every 5 seconds, but need to happen at the same time on all nodes. This means they need to happen every 5th second.

    let waitingroom_clone = waitingroom.clone();
    let evictions = async move {
        log::debug!("Starting timer");
        loop {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros();
            let sleep_time = 5_000_000 - (now % 5_000_000);
            tokio::time::sleep(tokio::time::Duration::from_micros(sleep_time as u64)).await;
            let mut waitingroom = waitingroom_clone.lock().unwrap();
            match DistributedWaitingRoom::eviction(&mut waitingroom) {
                Ok(_) => {}
                Err(err) => {
                    log::error!("Error in timer {}: {:?}", stringify!(evictions), err);
                }
            }
        }
    };

    tokio::join!(cleanup, receive_message, fault_detection, evictions).0;
}
