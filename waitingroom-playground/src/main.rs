// This module is for testing random things. Do not rely on it for anything.

use waitingroom_core::{time::DummyTimeProvider, WaitingRoomUserTriggered};
use waitingroom_distributed::{DistributedWaitingRoom, GeneralWaitingRoomSettings};

fn main() {
    let settings = GeneralWaitingRoomSettings {
        min_user_count: 1,
        max_user_count: 1,
        ticket_refresh_time: 6000,
        ticket_expiry_time: 15000,
        pass_expiry_time: 6000,
    };

    let dummy_time_provider = DummyTimeProvider::new();

    let mut waiting_room = DistributedWaitingRoom::new(settings, 1, dummy_time_provider);

    let ticket = waiting_room.join().unwrap();
    waiting_room.let_users_out_of_queue(1).unwrap();
    let checkin_result = waiting_room.check_in(ticket).unwrap();
    assert!(checkin_result.position_estimate == 0);
}
