use waitingroom_core::{settings::GeneralWaitingRoomSettings, time::DummyTimeProvider, WaitingRoomUserTriggered};

use crate::DistributedWaitingRoom;

#[test]
fn basic_test() {
    let settings = GeneralWaitingRoomSettings {
        min_user_count: 1,
        max_user_count: 1,
        ticket_refresh_time: 6000,
        ticket_expiry_time: 15000,
        pass_expiry_time: 6000,
    };

    let dummy_time_provider = DummyTimeProvider::new();

    let mut node = DistributedWaitingRoom::new(settings, 1, dummy_time_provider.clone());

    let ticket = node.join().unwrap();
    node.let_users_out_of_queue(1).unwrap();
    let checkin_result = node.check_in(ticket).unwrap();
    assert!(checkin_result.position_estimate == 0);
    let pass = node.leave(ticket).unwrap();
    let pass = if let Ok(new_pass) = node.validate_and_refresh_pass(pass) {
        new_pass
    } else {
        panic!("Invalid pass!");
    };

    dummy_time_provider.increase_by(6001);
    if node.validate_and_refresh_pass(pass).is_ok() {
        panic!("Pass should have been invalid, but wasn't!")
    };

    println!("All tests pass!");
}