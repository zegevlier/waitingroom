use std::fs::OpenOptions;

use fern::colors::ColoredLevelConfig;
use log::LevelFilter;
use waitingroom_core::{
    network::DummyNetwork,
    random::DeterministicRandomProvider,
    time::{DummyTimeProvider, TimeProvider},
};
use waitingroom_distributed::messages::NodeToNodeMessage;

mod checks;
mod simulation;
mod user;

use simulation::SimulationConfig;

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

    #[allow(unused)]
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open("output.log")
        .unwrap();

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
    debug_run(72);
    // testing_run(0..100);
}

#[allow(unused)]
fn debug_run(seed: u64) {
    let logging_level = LevelFilter::Debug;
    let time_provider = DummyTimeProvider::new();

    initialise_logging(&time_provider, logging_level);

    simulation::run(seed, &time_provider, SimulationConfig {});
}

#[allow(unused)]
fn testing_run(seed_range: std::ops::Range<u64>) {
    let logging_level = LevelFilter::Error;
    let time_provider = DummyTimeProvider::new();

    initialise_logging(&time_provider, logging_level);

    for seed in seed_range {
        time_provider.reset();
        log::error!("Seed: {}", seed);
        simulation::run(seed, &time_provider, SimulationConfig {});
    }
}

fn debug_print_qpid_info_for_nodes(nodes: &[Node]) {
    log::info!("Debug printing QPID states");
    for node in nodes.iter() {
        log::info!(
            "Node {}\t\tQPID parent: {:?}",
            node.get_node_id(),
            node.get_qpid_parent()
        );
        log::info!("Weight table:");
        log::info!("Neighbour\t\tWeight");
        for (neighbour, weight) in node.get_qpid_weight_table().all_weights() {
            log::info!("{}\t\t\t\t{}", neighbour, weight);
        }
    }
}
