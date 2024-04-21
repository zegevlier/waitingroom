// This module is for testing random things. Do not rely on it for anything.

use waitingroom_core::{
    network::DummyNetwork, time::DummyTimeProvider, WaitingRoomMessageTriggered,
    WaitingRoomUserTriggered,
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

    let dummy_time_provider = DummyTimeProvider::new();
    let dummy_network: DummyNetwork<NodeToNodeMessage> = DummyNetwork::new();

    let node1 = DistributedWaitingRoom::new(
        settings,
        1,
        dummy_time_provider.clone(),
        dummy_network.clone(),
    );

    let node2 = DistributedWaitingRoom::new(
        settings,
        2,
        dummy_time_provider.clone(),
        dummy_network.clone(),
    );

    let mut nodes = vec![node1, node2];

    let ticket = nodes[0].join().unwrap();
    process_messages(&mut nodes);
    nodes[1].qpid_delete_min().unwrap();
    process_messages(&mut nodes);
    let checkin_result = nodes[0].check_in(ticket).unwrap();

    assert!(checkin_result.position_estimate == 0);
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

    println!("All tests pass!");
}

fn process_messages(nodes: &mut [Node]) {
    while nodes.iter_mut().any(|n| n.receive_message().unwrap()) {}
}
