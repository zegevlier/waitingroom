use waitingroom_core::{network::LatencySetting, settings::GeneralWaitingRoomSettings, time::Time};

use super::UserBehaviour;

pub struct SimulationConfig {
    pub settings: GeneralWaitingRoomSettings,
    pub latency: LatencySetting,
    pub initial_node_count: usize,
    pub user_join_odds: u64,
    pub node_kill_odds: u64,
    pub check_consistency: bool,
    pub stop_at_time: Time,
    pub user_behaviour: UserBehaviour,
}
