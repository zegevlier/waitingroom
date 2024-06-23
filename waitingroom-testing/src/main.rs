use fern::colors::ColoredLevelConfig;
use log::LevelFilter;
use waitingroom_core::{
    network::{DummyNetwork, LatencySetting},
    random::DeterministicRandomProvider,
    settings::GeneralWaitingRoomSettings,
    time::{DummyTimeProvider, TimeProvider},
};
use waitingroom_distributed::messages::NodeToNodeMessage;

mod checks;
mod simulation;
mod user;

use simulation::{Simulation, SimulationConfig};

type Node = waitingroom_distributed::DistributedWaitingRoom<
    DummyTimeProvider,
    DeterministicRandomProvider,
    DummyNetwork<NodeToNodeMessage>,
>;

fn initialise_logging(time_provider: &DummyTimeProvider, logging_level: LevelFilter) {
    let colors = ColoredLevelConfig::new()
        .debug(fern::colors::Color::Cyan)
        .info(fern::colors::Color::Green)
        .warn(fern::colors::Color::Yellow)
        .error(fern::colors::Color::Red);

    // #[allow(unused)]
    // let file = OpenOptions::new()
    //     .write(true)
    //     .create(true)
    //     .truncate(true)
    //     .open("output.log")
    //     .unwrap();

    let time_provider_fern = time_provider.clone();
    fern::Dispatch::new()
        .format(move |out, message, record| {
            let start_length = record.target().len();
            let max_len = 30;
            let (target, target_padding) = if start_length > max_len {
                (&record.target()[start_length - max_len..], "".to_string())
            } else {
                (record.target(), " ".repeat(max_len - start_length))
            };
            let time = time_provider_fern.get_now_time();
            // Since it's much more likely to go wrong in the first 100 time steps, it does't matter as much if the rest is not aligned perfectly.
            let time_padding = " ".repeat(3_usize.saturating_sub(time.to_string().len()));
            out.finish(format_args!(
                "[{}{}][{}{}][{}] {}",
                target,
                target_padding,
                time,
                time_padding,
                colors.color(record.level()),
                message
            ))
        })
        .level(logging_level)
        .chain(std::io::stdout())
        // .chain(file)
        .level_for("waitingroom_core::random", log::LevelFilter::Info)
        .apply()
        .unwrap();
}

fn main() {
    let logging_level = LevelFilter::Debug;
    let time_provider = DummyTimeProvider::new();

    initialise_logging(&time_provider, logging_level);

    let config = SimulationConfig {
        settings: GeneralWaitingRoomSettings {
            min_user_count: 20,
            max_user_count: 25,
            ticket_refresh_time: 6000,
            ticket_expiry_time: 20000,
            pass_expiry_time: 0,
            fault_detection_period: 1000,
            fault_detection_timeout: 200,
            fault_detection_interval: 100,
            eviction_interval: 1000,
            cleanup_interval: 1000,
        },
        initial_node_count: 8,
        latency: LatencySetting::UniformRandom(10, 20),
        user_join_odds: 200,
        node_kill_odds: 1000,
        check_consistency: true,
        stop_at_time: 100000,
        user_behaviour: user::UserBehaviour {
            abandon_odds: 1000,
            pass_refresh_odds: 1000,
        },
    };

    let simulation = Simulation::new(config);

    match simulation.run(0) {
        Ok(results) => log::info!("Simulation completed successfully: {:?}", results),
        Err(e) => log::error!("Simulation failed: {:?}", e),
    }
}
