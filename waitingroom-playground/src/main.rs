// This module is for testing random things. Do not rely on it for anything.

use waitingroom_core::{
    network::{DummyNetwork, Latency},
    random::DeterministicRandomProvider,
    settings::GeneralWaitingRoomSettings,
    time::{DummyTimeProvider, Time},
    WaitingRoomMessageTriggered, WaitingRoomUserTriggered,
};

use waitingroom_distributed::{messages::NodeToNodeMessage, DistributedWaitingRoom, Weight};

type Node = DistributedWaitingRoom<
    DummyTimeProvider,
    DeterministicRandomProvider,
    DummyNetwork<NodeToNodeMessage>,
>;

fn main() {
    env_logger::init();

    let settings = GeneralWaitingRoomSettings {
        min_user_count: 1,
        max_user_count: 1,
        ticket_refresh_time: 6000,
        ticket_expiry_time: 15000,
        pass_expiry_time: 6000,
        fault_detection_period: 1000,
        fault_detection_timeout: 199,
        fault_detection_interval: 100,
        eviction_interval: 5000,
        cleanup_interval: 10000,
    };

    log::info!("Instantiating dummy time and network");
    let dummy_time_provider = DummyTimeProvider::new();
    let dummy_network = DummyNetwork::new(dummy_time_provider.clone(), Latency::Fixed(0));
    let random_provider = DeterministicRandomProvider::new(1);

    let mut nodes = vec![];

    let node_configs = [
        (0, vec![0, 1]),       // 0
        (0, vec![0, 1, 2]),    // 1
        (1, vec![1, 2, 3]),    // 2
        (2, vec![2, 3, 4, 5]), // 3
        (3, vec![3, 4]),       // 4
        (3, vec![3, 5]),       // 5
    ];

    log::info!("Creating {} waitingroom nodes", node_configs.len());
    for (node_id, (parent, neighbour_config)) in node_configs.iter().enumerate() {
        let mut node = DistributedWaitingRoom::new(
            settings,
            node_id,
            dummy_time_provider.clone(),
            random_provider.clone(),
            dummy_network.clone(),
        );
        node.testing_overwrite_qpid(
            Some(*parent),
            neighbour_config
                .iter()
                .map(|v| (*v, Weight::new(Time::MAX, 0, 0)))
                .collect(),
        );
        nodes.push(node);
    }

    dummy_time_provider.increase_by(2);
    let _ticket2 = nodes[5].join().unwrap();
    dummy_time_provider.increase_by(1);
    let _ticket3 = nodes[2].join().unwrap();
    dummy_time_provider.increase_by(1);
    let _ticket4 = nodes[1].join().unwrap();
    dummy_time_provider.increase_by(1);
    let _ticket5 = nodes[3].join().unwrap();
    dummy_time_provider.increase_by(1);
    let _ticket6 = nodes[0].join().unwrap();
    dummy_time_provider.increase_by(1);
    let _ticket7 = nodes[4].join().unwrap();

    process_messages(&mut nodes);

    log::info!("Debug printing QPID states");
    // for (i, node) in nodes.iter().enumerate() {
    //     println!("Node {}\nQPID parent: {}\t\t Self weight: {}", i, node.qpid_parent, node.qpid_self_weight);
    //     println!("Weight table:");
    //     println!("Neighbour\t\t Weight")
    //     for (neighbour, weight) in node.qpid_weight_table.iter() {
    //         println!("{}\t\t{}", neighbour, weight);
    //     }
    // }

    log::info!("Done!");
}

fn process_messages(nodes: &mut [Node]) {
    log::debug!("Processing messages");
    loop {
        let mut any_received = false;

        // We want to handle the messages on all nodes ~equally until no more messages are received.
        for node in nodes.iter_mut() {
            if node.receive_message().unwrap() {
                any_received = true;
            }
        }

        if !any_received {
            break;
        }
    }
}
