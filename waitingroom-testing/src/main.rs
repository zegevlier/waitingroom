use std::fs::OpenOptions;

use checks::assert_consistent_state;
use fern::colors::ColoredLevelConfig;
use waitingroom_core::{
    network::DummyNetwork,
    random::{DeterministicRandomProvider, RandomProvider},
    settings::GeneralWaitingRoomSettings,
    time::{DummyTimeProvider, TimeProvider},
    WaitingRoomMessageTriggered,
};
use waitingroom_distributed::messages::NodeToNodeMessage;

mod checks;

type Node = waitingroom_distributed::DistributedWaitingRoom<
    DummyTimeProvider,
    DeterministicRandomProvider,
    DummyNetwork<NodeToNodeMessage>,
>;

fn main() {
    let time_provider = DummyTimeProvider::new();

    // env_logger::init();
    let colors = ColoredLevelConfig::new()
        .debug(fern::colors::Color::Cyan)
        .info(fern::colors::Color::Green)
        .warn(fern::colors::Color::Yellow)
        .error(fern::colors::Color::Red);

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
        .level(log::LevelFilter::Debug)
        .chain(std::io::stdout())
        .chain(file)
        .level_for("waitingroom_core::random", log::LevelFilter::Info)
        .apply()
        .unwrap();

    let seed = 1;
    // We use a separate random provider for our decisions vs those of the network. This makes it easier to re-do tests with a modified node implementation.
    log::info!("Seed: {}", seed);
    let network_random_provider = DeterministicRandomProvider::new(seed);

    let node_random_provider =
        DeterministicRandomProvider::new(network_random_provider.random_u64());
    let latency =
        waitingroom_core::network::Latency::Random(1, 20, network_random_provider.clone());
    // let latency = waitingroom_core::network::Latency::Fixed(10);

    let network: DummyNetwork<NodeToNodeMessage> =
        DummyNetwork::new(time_provider.clone(), latency);

    let settings = GeneralWaitingRoomSettings {
        min_user_count: 20,
        max_user_count: 25,
        ticket_refresh_time: 6000,
        ticket_expiry_time: 20000,
        pass_expiry_time: 20000,
        fault_detection_interval: 1000,
        fault_detection_timeout: 200,
        fault_detection_period: 100,
        eviction_interval: 5000,
    };

    let mut nodes = Vec::new();

    let node_count = 5;

    for i in 0..node_count {
        let node = waitingroom_distributed::DistributedWaitingRoom::new(
            settings,
            i,
            time_provider.clone(),
            node_random_provider.clone(),
            network.clone(),
        );

        nodes.push(node);
    }

    // We initialize the network with the nodes.
    nodes[0].initialise_alone().unwrap();

    // We add the other nodes to the network.
    for node in nodes.iter_mut() {
        node.join_at(0).unwrap();
    }

    // Now we start the network running.
    loop {
        // Each iteration of the loop is one time step.
        time_provider.increase_by(1);

        process_messages(&mut nodes, &network_random_provider);

        // // We'll not mess with it in the first 100 time steps, since nodes are still joining.
        // if time_provider.get_now_time() < 40 {
        //     continue;
        // }
        // For now, don't do anything when there are in flight messages.
        if !network.is_empty() {
            continue;
        }

        // Now all messages are processed, we can do other fun things.
        // First, for good measure, we check if the network is still in a consistent state.
        assert_consistent_state(&nodes);

        if time_provider.get_now_time() > 100 {
            break;
        }
    }
}

fn process_messages(nodes: &mut [Node], random_provider: &DeterministicRandomProvider) {
    // We first process all the network messages that came in at this time step. We randomise the order in which the nodes process their messages.
    let mut node_indices: Vec<usize> = (0..nodes.len()).collect();

    let mut nodes_that_processed = Vec::new();
    loop {
        random_provider.shuffle(&mut node_indices);

        while let Some(node_index) = node_indices.pop() {
            let node = &mut nodes[node_index];
            if node.receive_message().unwrap() {
                nodes_that_processed.push(node_index);
            }
        }

        // This empties out the nodes_that_processed vector, and puts its old contents into node_indices.
        node_indices = std::mem::take(&mut nodes_that_processed);

        if node_indices.is_empty() {
            break;
        }
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
