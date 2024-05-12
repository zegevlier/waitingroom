// This module is for testing random things. Do not rely on it for anything.

use waitingroom_core::{
    network::{DummyNetwork, Latency},
    random::DeterministicRandomProvider,
    settings::GeneralWaitingRoomSettings,
    time::{DummyTimeProvider, Time},
    NodeId, WaitingRoomMessageTriggered, WaitingRoomTimerTriggered, WaitingRoomUserTriggered,
};
use waitingroom_distributed::{messages::NodeToNodeMessage, DistributedWaitingRoom};

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
        fault_detection_interval: 1000,
        fault_detection_timeout: 199,
        fault_detection_period: 100,
    };

    log::info!("Instantiating dummy time and network");
    let dummy_time_provider = DummyTimeProvider::new();
    let dummy_network = DummyNetwork::new(dummy_time_provider.clone(), Latency::Fixed(0));
    let random_provider = DeterministicRandomProvider::new(1);

    let mut nodes = vec![];

    let node_count = 2;
    log::info!("Creating {} waitingroom nodes", node_count);
    let init_weight_table: Vec<(NodeId, Time)> = (0..node_count).map(|v| (v, Time::MAX)).collect();
    for node_id in 0..node_count {
        let mut node = DistributedWaitingRoom::new(
            settings,
            node_id,
            dummy_time_provider.clone(),
            random_provider.clone(),
            dummy_network.clone(),
        );
        node.testing_overwrite_qpid(Some(1), init_weight_table.clone());
        nodes.push(node);
    }

    let ticket = nodes[0].join().unwrap();

    process_messages(&mut nodes);

    dummy_time_provider.increase_by(10);

    let ticket2 = nodes[1].join().unwrap();

    process_messages(&mut nodes);

    nodes.iter_mut().for_each(|node| {
        node.ensure_correct_user_count().unwrap();
    });

    process_messages(&mut nodes);

    let checkin_result = nodes[ticket.node_id].check_in(ticket).unwrap();

    assert!(checkin_result.position_estimate == 0);

    let checkin_result2 = nodes[ticket2.node_id].check_in(ticket2).unwrap();

    assert!(checkin_result2.position_estimate == 1);

    let pass = nodes[0].leave(ticket).unwrap();

    let _pass = if let Ok(new_pass) = nodes[0].validate_and_refresh_pass(pass) {
        new_pass
    } else {
        panic!("Invalid pass!");
    };

    dummy_time_provider.increase_by(300);

    nodes.iter_mut().for_each(|node| {
        node.ensure_correct_user_count().unwrap();
    });

    process_messages(&mut nodes);

    // Now, the other user SHOULDN'T be able to check in, because the first user is still on the site.

    let checkin_result2 = nodes[ticket2.node_id].check_in(ticket2).unwrap();

    assert!(checkin_result2.position_estimate == 1);

    let new_ticket = checkin_result2.new_ticket;

    // Now, we expire the pass and the first user should be able to check in again.
    dummy_time_provider.increase_by(6001);

    // First we need to do a cleanup, otherwise the pass won't be invalidated.
    nodes.iter_mut().for_each(|node| {
        node.cleanup().unwrap();
    });

    nodes.iter_mut().for_each(|node| {
        node.ensure_correct_user_count().unwrap();
    });

    process_messages(&mut nodes);

    let checkin_result2 = nodes[new_ticket.node_id].check_in(new_ticket).unwrap();

    assert!(checkin_result2.position_estimate == 0);

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
