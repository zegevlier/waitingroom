// This module is for testing random things. Do not rely on it for anything.

use waitingroom_core::{network::DummyNetwork, time::DummyTimeProvider, WaitingRoomUserTriggered};
use waitingroom_distributed::{
    messages::NodeToNodeMessage, DistributedWaitingRoom, GeneralWaitingRoomSettings,
};

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

    let mut node1 = DistributedWaitingRoom::new(
        settings,
        1,
        dummy_time_provider.clone(),
        dummy_network.clone(),
    );
    let mut node2 = DistributedWaitingRoom::new(
        settings,
        2,
        dummy_time_provider.clone(),
        dummy_network.clone(),
    );

    let ticket = node1.join().unwrap();
    node1.let_users_out_of_queue(1).unwrap();
    let checkin_result = node1.check_in(ticket).unwrap();
    assert!(checkin_result.position_estimate == 0);
    let pass = node1.leave(ticket).unwrap();
    let pass = if let Ok(new_pass) = node1.validate_and_refresh_pass(pass) {
        new_pass
    } else {
        panic!("Invalid pass!");
    };

    dummy_time_provider.increase_by(6001);
    if node1.validate_and_refresh_pass(pass).is_ok() {
        panic!("Pass should have been invalid, but wasn't!")
    };

    println!("All tests pass!");
}
