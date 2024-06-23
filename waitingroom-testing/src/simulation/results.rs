/// This struct is used to store the results of a simulation.
/// This should never be returned to the application, but instead be used `build` to create a `SimulationResults` struct.
pub(super) struct SimulationResultsBuilder {
    /// Number of users that entered the waiting room.
    total_users_added: usize,
    /// Number of users that actually left the waiting room (got a pass).
    total_users_left: usize,

    /// Number of nodes that were added to the network.
    /// This includes the initial node(s).
    total_nodes_added: usize,
    /// Number of nodes that were removed from the network.
    total_nodes_removed: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimulationResults {
    /// Number of users that entered the waiting room.
    pub total_users_added: usize,
    /// Number of users that actually left the waiting room (got a pass).
    pub total_users_left: usize,

    /// Number of nodes that were added to the network.
    /// This includes the initial node(s).
    pub total_nodes_added: usize,
    /// Number of nodes that were removed from the network.
    pub total_nodes_removed: usize,

    /// The normalised kendall tau distance between the actual order of users leaving the waiting room and the expected order.
    pub kendall_tau: f64,
}

impl SimulationResultsBuilder {
    pub fn new() -> Self {
        Self {
            total_users_added: 0,
            total_users_left: 0,
            total_nodes_added: 0,
            total_nodes_removed: 0,
        }
    }

    pub fn add_user(&mut self) {
        self.total_users_added += 1;
    }

    pub fn left_user(&mut self) {
        self.total_users_left += 1;
    }

    pub fn add_node(&mut self) {
        self.total_nodes_added += 1;
    }

    pub fn remove_node(&mut self) {
        self.total_nodes_removed += 1;
    }

    /// Build the simulation results.
    /// The kendall_tau parameter is the normalised kendall tau distance between the actual order of users leaving the waiting room and the expected order.
    pub fn build(self, kendall_tau: f64) -> SimulationResults {
        SimulationResults {
            total_users_added: self.total_users_added,
            total_users_left: self.total_users_left,
            total_nodes_added: self.total_nodes_added,
            total_nodes_removed: self.total_nodes_removed,
            kendall_tau,
        }
    }
}
