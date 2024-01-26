use foundations::settings::settings;

#[settings(impl_default = false)]
#[derive(Copy)]
pub struct BasicWaitingRoomSettings {
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

    /// The time in milliseconds between user count syncs across nodes.
    /// For the basic waiting room, this is a no-op.
    pub sync_user_counts_interval: u128,
    /// The time in milliseconds between cleanup operations.
    pub cleanup_interval: u128,
    /// The time in milliseconds between ensuring that correct number
    /// of users are on the site.
    pub ensure_correct_user_count_interval: u128,
}

impl Default for BasicWaitingRoomSettings {
    fn default() -> Self {
        Self {
            min_user_count: 20,
            max_user_count: 20,

            ticket_refresh_time: 20 * 1000,
            ticket_expiry_time: 45 * 1000,
            pass_expiry_time: 120 * 1000,

            sync_user_counts_interval: 10 * 1000,
            cleanup_interval: 10 * 1000,
            ensure_correct_user_count_interval: 10 * 1000,
        }
    }
}
