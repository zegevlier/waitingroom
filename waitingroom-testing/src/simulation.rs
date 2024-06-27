use waitingroom_core::{
    network::DummyNetwork,
    random::RandomProvider,
    settings::GeneralWaitingRoomSettings,
    time::{DummyTimeProvider, Time, TimeProvider},
    WaitingRoomError, WaitingRoomMessageTriggered, WaitingRoomTimerTriggered,
    WaitingRoomUserTriggered,
};
use waitingroom_distributed::messages::NodeToNodeMessage;

use crate::{
    checks::{check_consistent_state, InvariantCheckError},
    Node,
};

mod config;
mod random_providers;
mod results;
mod user;

pub use config::SimulationConfig;
use random_providers::RandomProviders;
pub use results::SimulationResults;
use results::SimulationResultsBuilder;
pub use user::{User, UserBehaviour};

pub struct Simulation {
    config: SimulationConfig,
}

pub struct RunningSimulation {
    node_settings: GeneralWaitingRoomSettings,
    time_provider: DummyTimeProvider,
    random_providers: RandomProviders,
    nodes: Vec<Node>,
    network: DummyNetwork<NodeToNodeMessage>,
    next_node_id: usize,
    results: SimulationResultsBuilder,
    users: Vec<User>,
}

impl RunningSimulation {
    fn new(config: &SimulationConfig, seed: u64) -> Self {
        let time_provider = DummyTimeProvider::new();
        let random_providers = RandomProviders::new(seed);

        let network: DummyNetwork<NodeToNodeMessage> = DummyNetwork::new(
            time_provider.clone(),
            config
                .latency
                .to_latency(Some(random_providers.network_random_provider().clone())),
        );

        Self {
            time_provider,
            random_providers,
            network,
            nodes: Vec::new(),
            next_node_id: 0,
            node_settings: config.settings,
            results: SimulationResultsBuilder::new(),
            users: Vec::new(),
        }
    }

    fn add_node(&mut self) -> Result<(), WaitingRoomError> {
        let mut node = waitingroom_distributed::DistributedWaitingRoom::new(
            self.node_settings,
            self.next_node_id,
            self.time_provider.clone(),
            self.random_providers.node_random_provider().clone(),
            self.network.clone(),
        );
        node.join_at(if self.nodes.is_empty() {
            0
        } else {
            self.nodes[0].get_node_id()
        })?;

        self.nodes.push(node);
        self.next_node_id += 1;
        self.results.add_node();
        Ok(())
    }

    fn initialise_network(&mut self, initial_node_count: usize) -> Result<(), WaitingRoomError> {
        assert!(
            initial_node_count > 0,
            "Initial node count must be greater than 0"
        );

        for _ in 0..initial_node_count {
            self.add_node()?;
        }

        Ok(())
    }

    fn tick_time(&mut self) {
        self.time_provider.increase_by(1);
    }

    fn get_now_time(&self) -> u128 {
        self.time_provider.get_now_time()
    }

    fn check_consistent_state(&self) -> Result<(), InvariantCheckError> {
        // TODO: Move this to be part of the running simulation.
        check_consistent_state(&self.nodes, &self.network)
    }

    fn debug_print(&self) {
        log::debug!("Debug printing node states");
        log::debug!("Time: {}", self.time_provider.get_now_time());
        log::debug!("Number of network messages: {}", self.network.len());
        log::debug!("{:?}", self.network.get_messages_mut());
        log::debug!("Number of users: {}", self.users.len());
        log::debug!("Number of nodes: {}\nNodes:", self.nodes.len());
        for node in self.nodes.iter() {
            log::debug!(
                "Node {}\t\tQPID parent: {:?}",
                node.get_node_id(),
                node.get_qpid_parent()
            );
            log::debug!(
                "True neighbours: {:?}",
                node.get_qpid_weight_table().get_true_neighbours()
            );
            log::debug!("Weight table:");
            log::debug!("Neighbour\t\tWeight");
            for (neighbour, weight) in node.get_qpid_weight_table().all_weights() {
                log::debug!("{}\t\t\t\t{:?}", neighbour, weight);
            }
        }
    }

    fn final_checks_and_results(
        self,
        check_consistency: bool,
    ) -> Result<SimulationResults, SimulationError> {
        // We'll check if we're in all the right states.
        // If we're not, this function will panic.
        if check_consistency {
            // TODO reenable
            // if let Err(error) = check_final_state(&self.nodes, &self.users) {
            //     self.debug_print();
            //     return Err(SimulationError::FinalStateCheck(error));
            // }
        }

        let (x, y): (Vec<_>, Vec<_>) = self
            .users
            .iter()
            .map(|u| (u.get_join_time(), u.get_eviction_time()))
            .filter(|(join_time, eviction_time)| join_time.is_some() && eviction_time.is_some())
            .unzip();

        let normalised_kendall_tau = kendall_tau::normalised_kendall_tau(&x, &y);

        let built_results = self
            .results
            .build(normalised_kendall_tau, self.time_provider.get_now_time());

        if built_results.total_users_added != built_results.total_users_left {
            log::error!(
                "Total users added ({}) does not match total users left ({})",
                built_results.total_users_added,
                built_results.total_users_left
            );
            self.debug_print();
            return Err(SimulationError::NotAllUsersLeft);
        }

        Ok(built_results)
    }

    fn process_messages(&mut self) -> Result<(), WaitingRoomError> {
        // We first process all the network messages that came in at this time step. We randomise the order in which the nodes process their messages.
        let mut node_indices: Vec<usize> = (0..self.nodes.len()).collect();

        let mut nodes_that_processed = Vec::new();
        loop {
            self.random_providers
                .network_random_provider()
                .shuffle(&mut node_indices);

            while let Some(node_index) = node_indices.pop() {
                let node = &mut self.nodes[node_index];
                if node.receive_message()? {
                    nodes_that_processed.push(node_index);
                }
            }

            if nodes_that_processed.is_empty() {
                break;
            }

            // This empties out the nodes_that_processed vector, and puts its old contents into node_indices.
            node_indices = std::mem::take(&mut nodes_that_processed);
        }

        Ok(())
    }

    fn call_timer_functions(&mut self) -> Result<(), WaitingRoomError> {
        let now = self.time_provider.get_now_time();

        if now % self.node_settings.cleanup_interval == 0 {
            // We'll call it on all nodes at the same time. This isn't strictly required for cleanup
            // but there's no reason not to.
            for node in self.nodes.iter_mut() {
                node.cleanup()?;
            }
        }

        if now % self.node_settings.eviction_interval == 0 {
            // It is important that we call the eviction function on all nodes at the same time.
            for node in self.nodes.iter_mut() {
                node.eviction()?;
            }
        }

        if now % self.node_settings.fault_detection_period == 0 {
            // We'll call it on all nodes at the same time. This isn't strictly required for fault detection
            for node in self.nodes.iter_mut() {
                node.fault_detection()?;
            }
        }

        Ok(())
    }

    fn do_user_actions(&mut self, _user_behaviour: &UserBehaviour) -> Result<(), WaitingRoomError> {
        let now = self.time_provider.get_now_time();

        let available_node_idxs = self
            .nodes
            .iter_mut()
            .enumerate()
            .filter(|(_, n)| n.get_qpid_parent().is_some())
            .map(|(idx, _)| idx)
            .collect::<Vec<_>>();

        let joining_available = !available_node_idxs.is_empty();

        if !joining_available {
            log::warn!("No nodes available for QPID operations!");
        }

        for user in 0..self.users.len() {
            let user = &mut self.users[user];
            if user.should_action(now) {
                match user.state() {
                    user::UserState::Joining => {
                        if !joining_available {
                            // We can't join right now, so we'll try again later.
                            continue;
                        }
                        let node_index = self.random_providers.user_random_provider().random_u64()
                            as usize
                            % available_node_idxs.len();
                        let ticket = self.nodes[available_node_idxs[node_index]].join()?;
                        user.join(ticket);
                    }
                    user::UserState::InQueue {
                        ticket,
                        next_action,
                    } => match next_action {
                        user::QueueAction::Refreshing => {
                            let node = match self
                                .nodes
                                .iter_mut()
                                .find(|n| n.get_node_id() == ticket.node_id)
                            {
                                Some(n) => n,
                                None => {
                                    if !joining_available {
                                        // We can't join at any other nodes at the moment, so we'll try again later.
                                        continue;
                                    }
                                    // The node we were on is gone, so we'll need to rejoin at another node.
                                    let node_index =
                                        self.random_providers.user_random_provider().random_u64()
                                            as usize
                                            % available_node_idxs.len();
                                    &mut self.nodes[available_node_idxs[node_index]]
                                }
                            };
                            let checkin_response = node.check_in(*ticket)?;
                            user.refresh_ticket(
                                checkin_response.new_ticket,
                                checkin_response.position_estimate,
                            );
                        }
                        user::QueueAction::Abandoning => todo!(),
                        user::QueueAction::Leaving => {
                            let node = match self
                                .nodes
                                .iter_mut()
                                .find(|n| n.get_node_id() == ticket.node_id)
                            {
                                Some(n) => n,
                                None => {
                                    // The node is gone, so we can't leave.
                                    // We'll need to rejoin at another node.
                                    user.return_to_refreshing();
                                    continue;
                                }
                            };
                            let pass = match node.leave(*ticket) {
                                Ok(pass) => pass,
                                Err(WaitingRoomError::TicketCannotLeaveYet) => {
                                    user.return_to_refreshing();
                                    continue;
                                }
                                Err(err) => return Err(err),
                            };
                            user.leave(pass);
                            self.results.left_user();
                        }
                    },
                    // We don't refresh tickets when users are on the site yet.
                    user::UserState::OnSite { .. } => todo!(),
                    user::UserState::Done { .. } => {}
                    user::UserState::Abandoned { .. } => {}
                }
            }
        }

        Ok(())
    }

    fn user_join(&mut self) {
        self.results.add_user();
        self.users.push(User::new());
    }

    fn kill_node(&mut self) {
        if self.nodes.len() > 1 {
            // We don't want to kill the last node.
            let node_index = self
                .random_providers
                .disturbance_random_provider()
                .random_u64() as usize
                % self.nodes.len();
            log::debug!("Killing node {}", node_index);
            self.network
                .remove_node(self.nodes[node_index].get_node_id());
            self.nodes.remove(node_index);
            self.results.remove_node();
        }
    }

    fn plan_disturbances(&self, count: usize, before: u128) -> Vec<u128> {
        let mut disturbance_timestamps = Vec::new();
        for _ in 0..count {
            disturbance_timestamps.push(
                self.random_providers
                    .disturbance_random_provider()
                    .random_u64() as u128
                    % before,
            );
        }
        disturbance_timestamps.sort();
        disturbance_timestamps
    }

    fn is_done(&self, end_timestamp: Time) -> bool {
        end_timestamp < self.get_now_time()
            && self
                .nodes
                .iter()
                .map(|n| n.in_queue_count() + n.in_queue_leaving_count())
                .sum::<usize>()
                == 0
    }
}

#[derive(Debug)]
pub enum SimulationError {
    WaitingRoom(WaitingRoomError),
    InvariantCheck(InvariantCheckError),
    SimulationTimeout,
    NotAllUsersLeft,
}

impl Simulation {
    pub fn new(config: SimulationConfig) -> Self {
        Self { config }
    }

    pub fn run(&self, seed: u64) -> Result<SimulationResults, SimulationError> {
        log::info!("Running simulation with seed {}", seed);

        let mut sim = RunningSimulation::new(&self.config, seed);

        match sim.initialise_network(self.config.initial_node_count) {
            Ok(_) => {}
            Err(err) => {
                log::error!("Failed to initialise network: {:?}", err);
                sim.debug_print();
                return Err(SimulationError::WaitingRoom(err));
            }
        }

        let mut user_join_timestamps = sim.plan_disturbances(
            self.config.total_user_count,
            self.config.time_until_cooldown,
        );
        let mut node_kill_timestamps = sim.plan_disturbances(
            self.config.nodes_killed_count,
            self.config.time_until_cooldown,
        );
        let mut node_join_timestamps = sim.plan_disturbances(
            self.config.nodes_added_count,
            self.config.time_until_cooldown,
        );

        // Now we start the network running.
        loop {
            sim.tick_time();
            let now = sim.get_now_time();

            if now == 61847 - 38 {
                sim.debug_print();
            }

            if now == 61847 - 380 {
                sim.debug_print();
            }

            // We'll check if we're in all the right states.
            // If we're not, this function will panic.
            if self.config.check_consistency {
                if let Err(error) = sim.check_consistent_state() {
                    log::error!("Error in invariant check: {:?}", error);
                    sim.debug_print();
                    return Err(SimulationError::InvariantCheck(error));
                }
            }

            match sim.process_messages() {
                Ok(_) => {}
                Err(err) => {
                    log::error!("Error processing messages: {:?}", err);
                    sim.debug_print();
                    return Err(SimulationError::WaitingRoom(err));
                }
            }

            match sim.call_timer_functions() {
                Ok(_) => {}
                Err(err) => {
                    log::error!("Error calling timer functions: {:?}", err);
                    sim.debug_print();
                    return Err(SimulationError::WaitingRoom(err));
                }
            };

            // Process user actions

            match sim.do_user_actions(&self.config.user_behaviour) {
                Ok(_) => {}
                Err(err) => {
                    log::error!("Error processing user actions: {:?}", err);
                    sim.debug_print();
                    return Err(SimulationError::WaitingRoom(err));
                }
            }

            // Add new users
            while !user_join_timestamps.is_empty() && user_join_timestamps[0] <= now {
                user_join_timestamps.remove(0);
                sim.user_join();
            }

            // Kill nodes
            while !node_kill_timestamps.is_empty() && node_kill_timestamps[0] <= now {
                sim.debug_print();
                node_kill_timestamps.remove(0);
                sim.kill_node();
            }

            // And add nodes
            if !node_join_timestamps.is_empty() && node_join_timestamps[0] <= now {
                node_join_timestamps.remove(0);
                match sim.add_node() {
                    Ok(_) => {}
                    Err(err) => {
                        log::error!("Error adding node to network: {:?}", err);
                    }
                }
            }

            // We wait until the required amount of time has passed and the network is done.
            if sim.get_now_time() == self.config.time_until_cooldown {
                sim.debug_print();
            }

            if sim.is_done(self.config.time_until_cooldown) {
                break;
            }

            if now >= self.config.time_until_cooldown + 5000 {
                let diff = now - self.config.time_until_cooldown - 5000;
                if diff % 100 == 0 {
                    sim.debug_print();
                }
                if diff > self.config.time_until_cooldown * 10 {
                    log::error!("Simulation took too long to complete");
                    return Err(SimulationError::SimulationTimeout);
                }
            }
        }

        log::info!("Simulation completed");
        sim.final_checks_and_results(self.config.check_consistency)
    }
}
