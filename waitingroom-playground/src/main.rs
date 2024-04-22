// This module is for testing random things. Do not rely on it for anything.

use waitingroom_core::{
    network::DummyNetwork,
    time::{DummyTimeProvider, Time},
    NodeId, WaitingRoomMessageTriggered, WaitingRoomUserTriggered,
};
use waitingroom_distributed::{
    messages::NodeToNodeMessage, DistributedWaitingRoom, GeneralWaitingRoomSettings,
};

type Node = DistributedWaitingRoom<DummyTimeProvider, DummyNetwork<NodeToNodeMessage>>;

fn main() {
    env_logger::init();

    let settings = GeneralWaitingRoomSettings {
        min_user_count: 1,
        max_user_count: 1,
        ticket_refresh_time: 6000,
        ticket_expiry_time: 15000,
        pass_expiry_time: 6000,
    };

    log::info!("Instantiating dummy time and network");
    let dummy_time_provider = DummyTimeProvider::new();
    let dummy_network: DummyNetwork<NodeToNodeMessage> = DummyNetwork::new();

    let mut nodes = vec![];

    let node_count = 2;
    log::info!("Creating {} waitingroom nodes", node_count);
    let init_weight_table: Vec<(NodeId, Time)> = (0..node_count).map(|v| (v, Time::MAX)).collect();
    for node_id in 0..node_count {
        let mut node = DistributedWaitingRoom::new(
            settings,
            node_id,
            dummy_time_provider.clone(),
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

    nodes[1].qpid_delete_min().unwrap();
    process_messages(&mut nodes);

    let checkin_result = nodes[ticket.node_id as usize].check_in(ticket).unwrap();
    assert!(checkin_result.position_estimate == 0);
    let checkin_result2 = nodes[ticket2.node_id as usize].check_in(ticket2).unwrap();
    assert!(checkin_result2.position_estimate == 1);

    let pass = nodes[0].leave(ticket).unwrap();

    let pass = if let Ok(new_pass) = nodes[0].validate_and_refresh_pass(pass) {
        new_pass
    } else {
        panic!("Invalid pass!");
    };

    dummy_time_provider.increase_by(6001);
    if nodes[0].validate_and_refresh_pass(pass).is_ok() {
        panic!("Pass should have been invalid, but wasn't!")
    };

    dummy_time_provider.increase_by(15000 - 6000);

    if nodes[ticket2.node_id as usize].check_in(ticket2).is_ok() {
        panic!("Should not have been able to check in after timeout!");
    }

    log::info!("Done");
}

fn process_messages(nodes: &mut [Node]) {
    log::debug!("Processing messages");
    while nodes.iter_mut().any(|n| n.receive_message().unwrap()) {}
}
