#[derive(Clone, Debug, Copy)]
pub struct GeneralWaitingRoomSettings {
    /// The minimum number of users that will be allowed on the site.
    /// If there are less than this number of users on the site,
    /// more users are let in.
    pub min_user_count: usize,
    /// The maximum number of users that will be allowed on the site.
    /// If there are more than this number of users on the site,
    /// users are not let in a number of times.
    pub max_user_count: usize,

    /// The time in milliseconds between ticket refreshes carried out by the client.
    pub ticket_refresh_time: u128,
    /// The time in milliseconds until a ticket expires if it is not refreshed.
    /// This should be greater than the ticket refresh time.
    pub ticket_expiry_time: u128,
    /// The time in milliseconds until a pass expires if it is not used.
    /// Passes are refreshed automatically when they are used.
    pub pass_expiry_time: u128,

    /// The interval in milliseconds between fault detection checks.
    pub fault_detection_period: u128,
    /// The time in milliseconds until a node is considered faulty if it has not responded.
    pub fault_detection_timeout: u128,
    /// The time in milliseconds between calls of the fault detection function.
    pub fault_detection_interval: u128,

    /// The time in milliseconds between evictions
    pub eviction_interval: u128,

    /// Time in milliseconds between calls to the cleanup function
    pub cleanup_interval: u128,
}

impl Default for GeneralWaitingRoomSettings {
    fn default() -> Self {
        Self {
            min_user_count: 20,
            max_user_count: 20,

            ticket_refresh_time: 20 * 1000,
            ticket_expiry_time: 45 * 1000,
            pass_expiry_time: 120 * 1000,

            fault_detection_period: 1000,
            fault_detection_timeout: 199,
            fault_detection_interval: 100,

            eviction_interval: 5000,
            cleanup_interval: 10000,
        }
    }
}
