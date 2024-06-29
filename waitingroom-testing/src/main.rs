use fern::colors::ColoredLevelConfig;
use log::LevelFilter;
use rayon::prelude::*;
use waitingroom_core::{
    network::{DummyNetwork, LatencySetting},
    random::DeterministicRandomProvider,
    settings::GeneralWaitingRoomSettings,
    time::{DummyTimeProvider, TimeProvider},
};
use waitingroom_distributed::messages::NodeToNodeMessage;

use std::io::Write;

mod checks;
mod simulation;

use simulation::{Simulation, SimulationConfig, UserBehaviour};

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
    let mut dis = fern::Dispatch::new()
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
        .level_for("waitingroom_core::random", log::LevelFilter::Info);

    if logging_level != LevelFilter::Debug {
        dis = dis.level_for("waitingroom_distributed", log::LevelFilter::Warn);
    }
    dis.apply().unwrap();
}

fn main() {
    // one_one_test();
    let logging_level = LevelFilter::Info;
    let time_provider = DummyTimeProvider::new();

    initialise_logging(&time_provider, logging_level);

    let mut config = SimulationConfig {
        settings: GeneralWaitingRoomSettings {
            target_user_count: 200,
            ticket_refresh_time: 600,
            ticket_expiry_time: 2000,
            pass_expiry_time: 0,
            fault_detection_period: 500,
            fault_detection_timeout: 200,
            fault_detection_interval: 100,
            eviction_interval: 1000,
            cleanup_interval: 1000,
        },
        initial_node_count: 8,
        latency: LatencySetting::UniformRandom(10, 20),
        total_user_count: 100,
        nodes_added_count: 1,
        nodes_killed_count: 1,
        check_consistency: false,
        time_until_cooldown: 100000,
        user_behaviour: UserBehaviour {
            abandon_odds: 1000,
            pass_refresh_odds: 1000,
        },
    };

    let possible_user_targets = [20, 100, 99999999];
    let possible_total_user_counts = [100, 500, 2500];
    let possible_node_killed_counts = [0, 1, 5];

    for target_user_count in possible_user_targets {
        for total_user_count in possible_total_user_counts.iter() {
            for nodes_killed_count in possible_node_killed_counts.iter() {
                let output_file = format!(
                    "results/target_{}_total_{}_killed_{}.jsonl",
                    target_user_count, total_user_count, nodes_killed_count
                );

                // if output file exists, skip
                if std::fs::metadata(&output_file).is_ok() {
                    log::info!("Skipping simulation with target_user_count: {}, total_user_count: {}, nodes_killed_count: {}", target_user_count, total_user_count, nodes_killed_count);
                    continue;
                }
                config.settings.target_user_count = target_user_count;
                config.total_user_count = *total_user_count;
                config.nodes_killed_count = *nodes_killed_count;
                config.nodes_added_count = *nodes_killed_count;
                let simulation = Simulation::new(config.clone());
                let results: Vec<_> = (0..1000)
                    .into_par_iter()
                    .filter_map(|seed| match simulation.run(seed) {
                        Ok(results) => {
                            log::info!("Simulation {} completed successfully: {:?}", seed, results);
                            Some(results)
                        }
                        Err(e) => {
                            log::error!("Simulation failed: {:?}", e);
                            None
                        }
                    })
                    .collect();

                let file = std::fs::File::create(output_file).unwrap();
                let mut writer = std::io::BufWriter::new(file);

                for result in results {
                    writeln!(writer, "{}", serde_json::to_string(&result).unwrap()).unwrap();
                }
            }
        }
    }

    let simulation = Simulation::new(config);
    dbg!(simulation.run(1).unwrap());

    // #[allow(clippy::useless_conversion)]
}

fn one_one_test() {
    let logging_level = LevelFilter::Info;
    let time_provider = DummyTimeProvider::new();

    initialise_logging(&time_provider, logging_level);

    let config = SimulationConfig {
        settings: GeneralWaitingRoomSettings {
            target_user_count: 20,
            ticket_refresh_time: 600,
            ticket_expiry_time: 2000,
            pass_expiry_time: 0,
            fault_detection_period: 500,
            fault_detection_timeout: 200,
            fault_detection_interval: 100,
            eviction_interval: 1000,
            cleanup_interval: 1000,
        },
        initial_node_count: 8,
        latency: LatencySetting::UniformRandom(10, 20),
        total_user_count: 500,
        nodes_added_count: 1,
        nodes_killed_count: 1,
        check_consistency: false,
        time_until_cooldown: 100000,
        user_behaviour: UserBehaviour {
            abandon_odds: 1000,
            pass_refresh_odds: 1000,
        },
    };

    let simulation = Simulation::new(config);
    (0..10000)
        .into_par_iter()
        .for_each(|seed| match simulation.run(seed) {
            Ok(results) => log::info!("Simulation {} completed successfully: {:?}", seed, results),
            Err(e) => log::error!("Simulation failed (seed: {}): {:?}", seed, e),
        });
    panic!("done");
}
