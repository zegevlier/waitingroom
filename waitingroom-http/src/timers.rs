use crate::settings::WaitingRoomTimerSettings;
use foundations::telemetry::{log, tracing};
use std::sync::Arc;
use std::sync::Mutex;
use tokio::time::{self, Duration};
use waitingroom_basic::BasicWaitingRoom;
use waitingroom_core::random::TrueRandomProvider;
use waitingroom_core::time::SystemTimeProvider;
use waitingroom_core::WaitingRoomTimerTriggered;

/// Run the waiting room operations that need to be triggered periodically.
/// Barring panics, this function will never return.
pub(crate) async fn timers(
    waitingroom: Arc<Mutex<BasicWaitingRoom<SystemTimeProvider, TrueRandomProvider>>>,
    waitingroom_settings: &WaitingRoomTimerSettings,
) {
    log::debug!("Setting up timers...");
    // TODO: Before we can use this for the distributed system, we need the ensure_correct_count to be triggered at the same time on all nodes.
    macro_rules! timer {
        ($name:ident, $interval:expr, $callback:expr) => {
            let mut $name = time::interval(Duration::from_millis($interval as u64));
            let waitingroom_clone = waitingroom.clone();
            let $name = async move {
                tracing::add_span_tags! { "timer" => stringify!($name)};
                log::debug!("Starting timer");
                loop {
                    $name.tick().await;
                    log::debug!("Timer triggered"; "timer" => stringify!($name));
                    let mut waitingroom = waitingroom_clone.lock().unwrap();
                    match $callback(&mut waitingroom) {
                        Ok(_) => {}
                        Err(err) => {
                            log::error!("Error in timer: {:?}", err; "timer" => stringify!($name));
                        }
                    }
                }
            };
        };
        () => {};
    }

    timer!(
        cleanup,
        waitingroom_settings.cleanup_interval,
        BasicWaitingRoom::cleanup
    );

    timer!(
        ensure_correct_count,
        waitingroom_settings.ensure_correct_user_count_interval,
        BasicWaitingRoom::ensure_correct_user_count
    );

    tokio::join!(cleanup, ensure_correct_count).0;
}
