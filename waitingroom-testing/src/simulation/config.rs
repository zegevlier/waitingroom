use waitingroom_core::{network::LatencySetting, settings::GeneralWaitingRoomSettings, time::Time};

use super::UserBehaviour;

#[derive(Clone, Debug)]
pub struct SimulationConfig {
    pub settings: GeneralWaitingRoomSettings,
    pub latency: LatencySetting,
    pub initial_node_count: usize,
    pub total_user_count: usize,
    pub nodes_killed_count: usize,
    pub nodes_added_count: usize,
    pub check_consistency: bool,
    pub time_until_cooldown: Time,
    pub user_behaviour: UserBehaviour,
}
