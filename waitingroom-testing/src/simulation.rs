use waitingroom_core::{
    network::DummyNetwork,
    random::{DeterministicRandomProvider, RandomProvider},
    settings::GeneralWaitingRoomSettings,
    time::{DummyTimeProvider, TimeProvider},
    WaitingRoomMessageTriggered, WaitingRoomTimerTriggered, WaitingRoomUserTriggered,
};
use waitingroom_distributed::messages::NodeToNodeMessage;

use crate::{
    checks::{assert_consistent_state, validate_results},
    debug_print_qpid_info_for_nodes,
    user::{User, UserAction},
    Node,
};

pub struct SimulationConfig {}

pub fn run(seed: u64, time_provider: &DummyTimeProvider, _simulation_config: SimulationConfig) {
    let node_count = 8;

    let settings = GeneralWaitingRoomSettings {
        min_user_count: 200,
        max_user_count: 250,
        ticket_refresh_time: 6000,
        ticket_expiry_time: 20000,
        pass_expiry_time: 0,
        fault_detection_period: 1000,
        fault_detection_timeout: 200,
        fault_detection_interval: 100,
        eviction_interval: 5000,
        cleanup_interval: 10000,
    };

    // let latency = waitingroom_core::network::Latency::Fixed(10);
    let latency = waitingroom_core::network::Latency::Random(1, 20, None);

    log::info!("Seed: {}", seed);
    // We use a separate random provider for the network, the nodes, and the disturbance.
    // All of these are seeded with the base random provider, which is seeded with the seed.
    // This ensures that everything is deterministic.
    let base_random_provider = DeterministicRandomProvider::new(seed);

    let network_random_provider =
        DeterministicRandomProvider::new(base_random_provider.random_u64());
    let node_random_provider = DeterministicRandomProvider::new(base_random_provider.random_u64());
    let disturbance_random_provider =
        DeterministicRandomProvider::new(base_random_provider.random_u64());

    let network: DummyNetwork<NodeToNodeMessage> = DummyNetwork::new(
        time_provider.clone(),
        latency.apply_random_provider(network_random_provider.clone()),
    );

    let mut nodes = Vec::new();

    for i in 0..node_count {
        let node = waitingroom_distributed::DistributedWaitingRoom::new(
            settings,
            i,
            time_provider.clone(),
            node_random_provider.clone(),
            network.clone(),
        );

        nodes.push(node);
    }

    // We initialize the network with the nodes.
    nodes[0].initialise_alone().unwrap();

    // We add the other nodes to the network.
    for node in nodes.iter_mut() {
        node.join_at(0).unwrap();
    }

    let mut past_initialisation = false;

    let mut users = Vec::new();

    // Now we start the network running.
    loop {
        // Each iteration of the loop is one time step.
        time_provider.increase_by(1);

        if [235083, 235085].contains(&time_provider.get_now_time()) {
            debug_print_qpid_info_for_nodes(&nodes);
        }

        process_messages(&mut nodes, &network_random_provider);

        // While the network is starting up, we just keep processing messages.
        // This is fine, because we have tests later that add and remove nodes, so we can test the network in a variety of states.
        if !network.is_empty() && !past_initialisation {
            continue;
        }

        if !past_initialisation {
            log::info!("Past initialisation");
            past_initialisation = true;
        }

        call_timer_functions(&mut nodes, time_provider, &settings);

        // We'll check if we're in all the right states.
        // If we're not, this function will panic.
        assert_consistent_state(&nodes, &network);

        do_user_actions(
            &mut users,
            &mut nodes,
            &disturbance_random_provider,
            time_provider,
        );

        if disturbance_random_provider.random_u64() % 200 == 0 {
            // We add a new user to a random node.
            let node_index = disturbance_random_provider.random_u64() as usize % nodes.len();
            let ticket = nodes[node_index].join().unwrap();

            users.push(User::new_refreshing(ticket));
        }

        // We'll stop the network after a number of time steps.
        if time_provider.get_now_time() > 1000000 {
            break;
        }
    }

    validate_results(&nodes, &users);
}

fn process_messages(nodes: &mut [Node], random_provider: &DeterministicRandomProvider) {
    // We first process all the network messages that came in at this time step. We randomise the order in which the nodes process their messages.
    let mut node_indices: Vec<usize> = (0..nodes.len()).collect();

    let mut nodes_that_processed = Vec::new();
    loop {
        random_provider.shuffle(&mut node_indices);

        while let Some(node_index) = node_indices.pop() {
            let node = &mut nodes[node_index];
            if node.receive_message().unwrap() {
                nodes_that_processed.push(node_index);
            }
        }

        // This empties out the nodes_that_processed vector, and puts its old contents into node_indices.
        node_indices = std::mem::take(&mut nodes_that_processed);

        if node_indices.is_empty() {
            break;
        }
    }
}

fn call_timer_functions(
    nodes: &mut [Node],
    time_provider: &DummyTimeProvider,
    settings: &GeneralWaitingRoomSettings,
) {
    let now = time_provider.get_now_time();

    if now % settings.cleanup_interval == 0 {
        // We'll call it on all nodes at the same time. This isn't strictly required for cleanup
        // but there's no reason not to.
        for node in nodes.iter_mut() {
            node.cleanup().unwrap();
        }
    }

    if now % settings.eviction_interval == 0 {
        // It is important that we call the eviction function on all nodes at the same time.
        for node in nodes.iter_mut() {
            node.eviction().unwrap();
        }
    }

    if now % settings.fault_detection_period == 0 {
        // We'll call it on all nodes at the same time. This isn't strictly required for fault detection
        for node in nodes.iter_mut() {
            node.fault_detection().unwrap();
        }
    }
}

fn do_user_actions(
    users: &mut [User],
    nodes: &mut [Node],
    _random_provider: &DeterministicRandomProvider, // We don't use this yet, but once we add a bit more randomness to the user actions, we will.
    time_provider: &DummyTimeProvider,
) {
    let now = time_provider.get_now_time();

    let mut i = 0;
    while i < users.len() {
        let user = &mut users[i];

        if user.should_action(now) {
            match user.get_action() {
                UserAction::Refresh => {
                    let ticket = user.take_ticket();
                    let checkin_response = nodes[ticket.node_id].check_in(ticket).unwrap();
                    user.refresh_ticket(
                        checkin_response.position_estimate,
                        checkin_response.new_ticket,
                    );
                }
                UserAction::Leave => {
                    let ticket = user.take_ticket();
                    let pass = nodes[ticket.node_id].leave(ticket).unwrap();
                    user.set_pass(pass);
                    // TODO: Add that the user can refresh the pass
                }
                UserAction::Done => {}
            }
        }
        i += 1;
    }
}
