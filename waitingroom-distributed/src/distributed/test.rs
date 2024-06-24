use waitingroom_core::{
    network::{DummyNetwork, Latency},
    random::{DeterministicRandomProvider, RandomProvider},
    settings::GeneralWaitingRoomSettings,
    time::{DummyTimeProvider, Time},
    NodeId, WaitingRoomMessageTriggered, WaitingRoomTimerTriggered, WaitingRoomUserTriggered,
};

use test_log::test;

use crate::{messages::NodeToNodeMessage, weight_table::Weight, DistributedWaitingRoom};

type Node = DistributedWaitingRoom<
    DummyTimeProvider,
    DeterministicRandomProvider,
    DummyNetwork<NodeToNodeMessage>,
>;

#[test]
fn basic_test() {
    let settings = GeneralWaitingRoomSettings {
        min_user_count: 1,
        max_user_count: 1,
        ticket_refresh_time: 6000,
        ticket_expiry_time: 15000,
        pass_expiry_time: 6000,
        ..Default::default()
    };

    let dummy_time_provider = DummyTimeProvider::new();
    let deterministic_random_provider = DeterministicRandomProvider::new(1);
    let dummy_network = DummyNetwork::new(dummy_time_provider.clone(), Latency::Fixed(0));

    let mut node = DistributedWaitingRoom::new(
        settings,
        1,
        dummy_time_provider.clone(),
        deterministic_random_provider,
        dummy_network,
    );

    node.testing_overwrite_qpid(Some(1), vec![(1, (Time::MAX, 0))]);

    let ticket = node.join().unwrap();
    node.qpid_delete_min().unwrap();
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

#[test]
fn simple_distributed_test() {
    let settings = GeneralWaitingRoomSettings {
        min_user_count: 1,
        max_user_count: 1,
        ticket_refresh_time: 6000,
        ticket_expiry_time: 15000,
        pass_expiry_time: 6000,
        ..Default::default()
    };

    log::info!("Instantiating dummy time and network");
    let dummy_time_provider = DummyTimeProvider::new();
    let dummy_random_provider = DeterministicRandomProvider::new(1);
    let dummy_network = DummyNetwork::new(dummy_time_provider.clone(), Latency::Fixed(0));

    let mut nodes = vec![];

    let node_count = 2;
    log::info!("Creating {} waitingroom nodes", node_count);
    let init_weight_table: Vec<(NodeId, Weight)> =
        (0..node_count).map(|v| (v, (Time::MAX, 0))).collect();
    for node_id in 0..node_count {
        let mut node = DistributedWaitingRoom::new(
            settings,
            node_id,
            dummy_time_provider.clone(),
            dummy_random_provider.clone(),
            dummy_network.clone(),
        );
        node.testing_overwrite_qpid(Some(1), init_weight_table.clone());
        nodes.push(node);
    }

    let ticket = nodes[0].join().unwrap();

    process_messages(&mut nodes, 10);

    dummy_time_provider.increase_by(10);

    let ticket2 = nodes[1].join().unwrap();
    process_messages(&mut nodes, 10);

    nodes[1].qpid_delete_min().unwrap();
    process_messages(&mut nodes, 10);

    let checkin_result = nodes[ticket.node_id].check_in(ticket).unwrap();
    assert!(checkin_result.position_estimate == 0);
    let checkin_result2 = nodes[ticket2.node_id].check_in(ticket2).unwrap();
    assert!(checkin_result2.position_estimate == 1);

    let pass = nodes[0].leave(ticket).unwrap();

    let pass = if let Ok(new_pass) = nodes[0].validate_and_refresh_pass(pass) {
        new_pass
    } else {
        panic!("Invalid pass!");
    };

    dummy_time_provider.increase_by(6001);
    if nodes[0].validate_and_refresh_pass(pass).is_ok() {
        panic!("Pass should have been invalid, but wasn't!")
    };

    dummy_time_provider.increase_by(15000 - 6000);

    if nodes[ticket2.node_id].check_in(ticket2).is_ok() {
        panic!("Should not have been able to check in after timeout!");
    }

    log::info!("Done");
}

#[test]
fn check_letting_users_out_of_queue_on_timer() {
    let settings = GeneralWaitingRoomSettings {
        min_user_count: 1,
        max_user_count: 1,
        ticket_refresh_time: 6000,
        ticket_expiry_time: 15000,
        pass_expiry_time: 6000,
        ..Default::default()
    };

    log::info!("Instantiating dummy time and network");
    let dummy_time_provider = DummyTimeProvider::new();
    let dummy_network = DummyNetwork::new(dummy_time_provider.clone(), Latency::Fixed(0));
    let random_provider = DeterministicRandomProvider::new(1);

    let mut nodes = vec![];

    let node_count = 2;
    log::info!("Creating {} waitingroom nodes", node_count);
    let init_weight_table: Vec<(NodeId, Weight)> =
        (0..node_count).map(|v| (v, (Time::MAX, 0))).collect();
    for node_id in 0..node_count {
        let mut node = DistributedWaitingRoom::new(
            settings,
            node_id,
            dummy_time_provider.clone(),
            random_provider.clone(),
            dummy_network.clone(),
        );
        node.testing_overwrite_qpid(Some(1), init_weight_table.clone());
        nodes.push(node);
    }

    let ticket = nodes[0].join().unwrap();

    process_messages(&mut nodes, 10);

    dummy_time_provider.increase_by(10);

    let ticket2 = nodes[1].join().unwrap();

    process_messages(&mut nodes, 10);

    nodes.iter_mut().for_each(|node| {
        node.eviction().unwrap();
    });

    process_messages(&mut nodes, 10);

    let checkin_result = nodes[ticket.node_id].check_in(ticket).unwrap();

    assert!(checkin_result.position_estimate == 0);

    let checkin_result2 = nodes[ticket2.node_id].check_in(ticket2).unwrap();

    assert!(checkin_result2.position_estimate == 1);

    let pass = nodes[0].leave(ticket).unwrap();

    let _pass = if let Ok(new_pass) = nodes[0].validate_and_refresh_pass(pass) {
        new_pass
    } else {
        panic!("Invalid pass!");
    };

    dummy_time_provider.increase_by(300);

    nodes.iter_mut().for_each(|node| {
        node.eviction().unwrap();
    });

    process_messages(&mut nodes, 10);

    // Now, the other user SHOULDN'T be able to check in, because the first user is still on the site.

    let checkin_result2 = nodes[ticket2.node_id].check_in(ticket2).unwrap();

    assert!(checkin_result2.position_estimate == 1);

    let new_ticket = checkin_result2.new_ticket;

    // Now, we expire the pass and the first user should be able to check in again.
    dummy_time_provider.increase_by(6001);

    // First we need to do a cleanup, otherwise the pass won't be invalidated.
    nodes.iter_mut().for_each(|node| {
        node.cleanup().unwrap();
    });

    nodes.iter_mut().for_each(|node| {
        node.eviction().unwrap();
    });

    process_messages(&mut nodes, 10);

    let checkin_result2 = nodes[new_ticket.node_id].check_in(new_ticket).unwrap();

    assert!(checkin_result2.position_estimate == 0);

    log::info!("Done!");
}

fn process_messages(nodes: &mut [Node], max_number: usize) -> bool {
    log::debug!("Processing messages");
    for _ in 0..max_number {
        if !nodes.iter_mut().any(|n| n.receive_message().unwrap()) {
            return true;
        }
    }
    false
}

#[test]
fn simple_fault_test() {
    let settings = GeneralWaitingRoomSettings {
        fault_detection_period: 1000,
        fault_detection_timeout: 199,
        fault_detection_interval: 100,
        ..Default::default()
    };

    log::info!("Instantiating dummy time and network");
    let dummy_time_provider = DummyTimeProvider::new();
    let dummy_random_provider = DeterministicRandomProvider::new(1);
    let dummy_network = DummyNetwork::new(dummy_time_provider.clone(), Latency::Fixed(20)); // 20ms latency

    let mut nodes = vec![];

    let node_count = 2;
    log::info!("Creating {} waitingroom nodes", node_count);
    let init_weight_table: Vec<(NodeId, Weight)> =
        (0..node_count).map(|v| (v, (Time::MAX, 0))).collect();
    for node_id in 0..node_count {
        let mut node = DistributedWaitingRoom::new(
            settings,
            node_id,
            dummy_time_provider.clone(),
            dummy_random_provider.clone(),
            dummy_network.clone(),
        );
        node.testing_overwrite_qpid(Some(1), init_weight_table.clone());
        nodes.push(node);
    }

    dummy_time_provider.increase_by(1001);

    // These should send a message to the other node each
    nodes[0].fault_detection().unwrap();
    nodes[1].fault_detection().unwrap();

    assert!(dummy_network.len() == 2);

    dummy_time_provider.increase_by(20);

    process_messages(&mut nodes, 10);

    assert!(dummy_network.len() == 2, "They should both have replied.");

    dummy_time_provider.increase_by(20);
    process_messages(&mut nodes, 10);

    assert!(dummy_network.is_empty(), "the messages should be processed");

    // These are before the interval, so no messages should be sent
    nodes[0].fault_detection().unwrap();
    nodes[1].fault_detection().unwrap();

    assert!(dummy_network.is_empty(), "No messages should be sent");

    // Now, we wait for the interval to pass
    dummy_time_provider.increase_by(1000);
    // And we make node 1 "fail" (we remove it from the nodes list so it can't send messages)
    nodes.remove(1);

    // Now do a fault check on 0, which should detect that 1 is not responding
    nodes[0].fault_detection().unwrap();

    process_messages(&mut nodes, 10); // This doesn't do anything, because node 1 is not in the network anymore
    assert!(
        dummy_network.len() == 1,
        "The old message shouldn't be picked up"
    );

    // Now we wait before the timeout
    dummy_time_provider.increase_by(100);

    nodes[0].fault_detection().unwrap(); // This is before the timeout, so it should not do anything

    process_messages(&mut nodes, 10);

    // The old message will still be there, since it's not picked up by the other node, but no new messages should be sent
    assert!(dummy_network.len() == 1, "No new messages should be sent");

    // Now we wait for the timeout
    dummy_time_provider.increase_by(100);
    nodes[0].fault_detection().unwrap(); // This is after the timeout, so node 0 should consider node 1 faulty.

    // TODO: Actually check this. This is currently not possible to check, but will be once I implement fault recovery.
    // However, we can see that the console logs "node 1 is down", so the fault detection works, at least in this test case.

    log::info!("Done");
}

#[test]
fn multi_node() {
    // This test is a regression test for a bug that was found in the distributed waiting room.
    let settings = GeneralWaitingRoomSettings {
        ..Default::default()
    };

    log::info!("Instantiating dummy time and network");
    let dummy_time_provider = DummyTimeProvider::new();
    let dummy_network = DummyNetwork::new(dummy_time_provider.clone(), Latency::Fixed(0));
    let random_provider = DeterministicRandomProvider::new(1);

    let mut nodes = vec![];

    let node_configs = [
        (0, vec![0, 1]),       // 0
        (0, vec![0, 1, 2]),    // 1
        (1, vec![1, 2, 3]),    // 2
        (2, vec![2, 3, 4, 5]), // 3
        (3, vec![3, 4]),       // 4
        (3, vec![3, 5]),       // 5
    ];

    log::info!("Creating {} waitingroom nodes", node_configs.len());
    for (node_id, (parent, neighbour_config)) in node_configs.iter().enumerate() {
        let mut node = DistributedWaitingRoom::new(
            settings,
            node_id,
            dummy_time_provider.clone(),
            random_provider.clone(),
            dummy_network.clone(),
        );
        node.testing_overwrite_qpid(
            Some(*parent),
            neighbour_config
                .iter()
                .map(|v| (*v, (Time::MAX, 0)))
                .collect(),
        );
        nodes.push(node);
    }

    dummy_time_provider.increase_by(2);
    let _ticket2 = nodes[5].join().unwrap();
    dummy_time_provider.increase_by(1);
    let _ticket3 = nodes[2].join().unwrap();
    dummy_time_provider.increase_by(1);
    let _ticket4 = nodes[1].join().unwrap();
    dummy_time_provider.increase_by(1);
    let _ticket5 = nodes[3].join().unwrap();
    dummy_time_provider.increase_by(1);
    let _ticket6 = nodes[0].join().unwrap();
    dummy_time_provider.increase_by(1);
    let _ticket7 = nodes[4].join().unwrap();

    assert!(
        process_messages(&mut nodes, 100),
        "Stuck in an infinite message loop"
    );

    debug_print_qpid_info_for_nodes(nodes.as_slice());
    verify_qpid_invariant(nodes.as_slice());

    for i in 0..nodes.len() {
        log::debug!("\n\n\n\n\n");
        log::debug!("Deleting min number {}", i);
        nodes[1].qpid_delete_min().unwrap();
        assert!(
            process_messages(&mut nodes, 100),
            "Stuck in an infinite message loop"
        );

        debug_print_qpid_info_for_nodes(nodes.as_slice());
        verify_qpid_invariant(nodes.as_slice());
    }
}

fn debug_print_qpid_info_for_nodes(nodes: &[Node]) {
    log::info!("Debug printing QPID states");
    for node in nodes.iter() {
        log::info!("Node {}\nQPID parent: {:?}", node.node_id, node.qpid_parent);
        log::info!("Weight table:");
        log::info!("Neighbour\t\tWeight");
        for (neighbour, weight) in node.qpid_weight_table.all_weights() {
            log::info!("{}\t\t\t\t{:?}", neighbour, weight);
        }
    }
}

fn verify_qpid_invariant(nodes: &[Node]) {
    for v in nodes.iter() {
        let parent_v = v.qpid_parent.unwrap();

        let w_v_parent_v = v.qpid_weight_table.compute_weight(parent_v);

        let w_v = v.qpid_weight_table.get_weight(v.node_id).unwrap();

        let mut min_weight = w_v;

        // Now we look at all nodes, check if their parent is the current node, and if so, check if their weight is less than the parent weight
        for x in nodes.iter() {
            if x.qpid_parent.unwrap() == v.node_id {
                let w_x_v = x.qpid_weight_table.compute_weight(v.node_id);
                min_weight = min_weight.min(w_x_v);
            }
        }

        // Now we assert the invariant
        assert_eq!(
            min_weight, w_v_parent_v,
            "Invariant failed for node {}",
            v.node_id
        );
    }
}

fn ensure_only_single_root(nodes: &[Node]) {
    let mut root_count = 0;
    for node in nodes {
        if node.qpid_parent == Some(node.node_id) {
            root_count += 1;
        }
    }
    assert_eq!(root_count, 1, "There should be exactly one root node");
}

#[test]
fn mid_eviction_time_root_change() {
    let settings = GeneralWaitingRoomSettings {
        eviction_interval: 1000,
        min_user_count: 1,
        max_user_count: 1,
        ..Default::default()
    };

    log::info!("Instantiating dummy time and network");
    let dummy_time_provider = DummyTimeProvider::new();
    let dummy_random_provider = DeterministicRandomProvider::new(1);
    let dummy_network = DummyNetwork::new(dummy_time_provider.clone(), Latency::Fixed(20)); // 20ms latency

    let mut nodes = vec![];

    let node_count = 2;
    log::info!("Creating {} waitingroom nodes", node_count);
    let init_weight_table: Vec<(NodeId, Weight)> = (0..node_count).map(|v| (v, (Time::MAX, 0))).collect();
    for node_id in 0..node_count {
        let mut node = DistributedWaitingRoom::new(
            settings,
            node_id,
            dummy_time_provider.clone(),
            dummy_random_provider.clone(),
            dummy_network.clone(),
        );
        node.testing_overwrite_qpid(Some(1), init_weight_table.clone());
        nodes.push(node);
    }

    dummy_time_provider.increase_by(1000);
    // Now an eviction happens.
    log::debug!("Doing first eviction");
    nodes[0].eviction().unwrap();
    nodes[1].eviction().unwrap();
    // This eviction will do nothing.
    process_messages(&mut nodes, 10);

    // Now we skip ahead until just before we would do another eviction.
    dummy_time_provider.increase_by(1000 - 30);
    // We first get it into a state where the root is mid-change.
    let ticket = nodes[1].join().unwrap(); // We do this by having the user join node 1. Since the current root is node 0, this will trigger a root change.
    dummy_time_provider.increase_by(20); // We wait until the first message arrives.
    nodes[0].receive_message().unwrap(); // We process the message that triggers the root change.
                                         // Now, we don't wait until the second message arrives, but instead we trigger the eviction.
    dummy_time_provider.increase_by(10);
    log::debug!("Doing second eviction");
    nodes[0].eviction().unwrap();
    nodes[1].eviction().unwrap();
    // Now we wait until the message arrives, then we check if we're doing an eviction on node 1.
    dummy_time_provider.increase_by(10);
    process_messages(&mut nodes, 10);
    dummy_time_provider.increase_by(20);
    process_messages(&mut nodes, 10);
    dummy_time_provider.increase_by(20);
    process_messages(&mut nodes, 10);
    // Now, we should have done an eviction on node 1.
    // We check this by checking if the user can leave.
    let position = nodes[1].check_in(ticket).unwrap().position_estimate;
    assert_eq!(
        position, 0,
        "User should be first in line, but is at position {}",
        position
    );
}

#[test]
fn membership_basic_add_remove() {
    let settings = GeneralWaitingRoomSettings {
        ..Default::default()
    };

    log::info!("Instantiating dummy time and network");
    let dummy_time_provider = DummyTimeProvider::new();
    let dummy_random_provider = DeterministicRandomProvider::new(1);
    let dummy_network = DummyNetwork::new(dummy_time_provider.clone(), Latency::Fixed(20));

    let mut nodes = vec![];

    let node_count = 10;
    log::info!("Creating {} waitingroom nodes", node_count);
    for node_id in 0..node_count {
        let node = DistributedWaitingRoom::new(
            settings,
            node_id,
            dummy_time_provider.clone(),
            dummy_random_provider.clone(),
            dummy_network.clone(),
        );
        nodes.push(node);
    }

    // Now we initialise the node number 0 as the "first" node.
    nodes[0].initialise_alone().unwrap();

    // Now we add the other nodes, one by one.
    for i in 1..node_count {
        log::debug!("Adding node {}", i);
        // nodes[0].add_node(i).unwrap();
        nodes[i].join_at(0).unwrap();

        dummy_time_provider.increase_by(20);
        process_messages(&mut nodes, 10);
        dummy_time_provider.increase_by(20);
        process_messages(&mut nodes, 10);
        dummy_time_provider.increase_by(20);
        process_messages(&mut nodes, 10);

        debug_print_qpid_info_for_nodes(&nodes[..i + 1]);

        verify_qpid_invariant(&nodes[..i + 1]);
        ensure_only_single_root(&nodes[..i + 1]);
    }

    // Now, we'll delete a leaf node.
    log::debug!("Deleting node 3");
    let _node_3 = nodes.remove(3);
    // We could wait for fault detection to find it, but in this test we'll just kick it manually.
    // Node 5 (index 4) has found that it's faulty and will remove it.
    log::debug!("Kicking node 3");
    nodes[4].remove_node(3).unwrap();
    // We'll do some rounds of message processing to make sure the message is processed.
    dummy_time_provider.increase_by(20);
    process_messages(&mut nodes, 10);

    debug_print_qpid_info_for_nodes(&nodes);
    verify_qpid_invariant(&nodes);
    ensure_only_single_root(&nodes);

    // Now, we'll delete a node that is not a leaf node.
    // Let's do node 4, which has 3 children.
    log::debug!("Deleting node 4");
    let _node_4 = nodes.remove(3); // Index 3 is node 4, since we removed node 3.
                                   // We could wait for fault detection to find it, but in this test we'll just kick it manually.
                                   // Node 5 (index 3) has found that it's faulty and will remove it.
    log::debug!("Kicking node 4");
    nodes[3].remove_node(4).unwrap();
    // We'll do some rounds of message processing to make sure the message is processed.
    dummy_time_provider.increase_by(20);
    process_messages(&mut nodes, 10);
    dummy_time_provider.increase_by(20);
    process_messages(&mut nodes, 10);

    debug_print_qpid_info_for_nodes(&nodes);
    verify_qpid_invariant(&nodes);
    ensure_only_single_root(&nodes);
}

#[test]
fn membership_nonempty() {
    let settings = GeneralWaitingRoomSettings {
        ..Default::default()
    };

    log::info!("Instantiating dummy time and network");
    let dummy_time_provider = DummyTimeProvider::new();
    let dummy_random_provider = DeterministicRandomProvider::new(1);
    let dummy_network = DummyNetwork::new(dummy_time_provider.clone(), Latency::Fixed(20));

    let mut nodes = vec![];

    let node_count = 10;
    log::info!("Creating {} waitingroom nodes", node_count);
    for node_id in 0..node_count {
        let node = DistributedWaitingRoom::new(
            settings,
            node_id,
            dummy_time_provider.clone(),
            dummy_random_provider.clone(),
            dummy_network.clone(),
        );
        nodes.push(node);
    }

    // Now we initialise the node number 0 as the "first" node.
    nodes[0].initialise_alone().unwrap();

    let mut tickets = vec![];

    // We'll join at that node too, for funzies
    let ticket = nodes[0].join().unwrap();
    tickets.push(ticket);

    // Now we add the other nodes, one by one.
    for i in 1..node_count {
        log::debug!("Adding node {}", i);
        // nodes[0].add_node(i).unwrap();
        nodes[i].join_at(0).unwrap();

        dummy_time_provider.increase_by(20);
        process_messages(&mut nodes, 10);
        dummy_time_provider.increase_by(20);
        process_messages(&mut nodes, 10);
        dummy_time_provider.increase_by(20);
        process_messages(&mut nodes, 10);

        debug_print_qpid_info_for_nodes(&nodes[..i + 1]);

        verify_qpid_invariant(&nodes[..i + 1]);
        ensure_only_single_root(&nodes[..i + 1]);

        // And we'll join at a random node
        let node_to_join = dummy_random_provider.random_u64() as usize % (i + 1);
        log::debug!("Joining at node {}", node_to_join);
        let ticket = nodes[node_to_join].join().unwrap();
        tickets.push(ticket);
        dummy_time_provider.increase_by(20);
        process_messages(&mut nodes, 10);
        dummy_time_provider.increase_by(20);
        process_messages(&mut nodes, 10);
        dummy_time_provider.increase_by(20);
        process_messages(&mut nodes, 10);
        dummy_time_provider.increase_by(20);
        process_messages(&mut nodes, 10);
    }

    // Now, we'll delete a leaf node.
    log::debug!("Deleting node 3");
    let _node_3 = nodes.remove(3);
    // We could wait for fault detection to find it, but in this test we'll just kick it manually.
    // Node 5 (index 4) has found that it's faulty and will remove it.
    log::debug!("Kicking node 3");
    nodes[4].remove_node(3).unwrap();
    // We'll do some rounds of message processing to make sure the message is processed.
    dummy_time_provider.increase_by(20);
    process_messages(&mut nodes, 10);
    dummy_time_provider.increase_by(20);
    process_messages(&mut nodes, 10);

    debug_print_qpid_info_for_nodes(&nodes);
    verify_qpid_invariant(&nodes);
    ensure_only_single_root(&nodes);

    // Now, we'll delete a node that is not a leaf node.
    // Let's do node 4, which has 3 children.
    log::debug!("Deleting node 4");
    let _node_4 = nodes.remove(3); // Index 3 is node 4, since we removed node 3.
                                   // We could wait for fault detection to find it, but in this test we'll just kick it manually.
                                   // Node 5 (index 3) has found that it's faulty and will remove it.
    log::debug!("Kicking node 4");
    nodes[3].remove_node(4).unwrap();
    // We'll do some rounds of message processing to make sure the message is processed.
    dummy_time_provider.increase_by(20);
    process_messages(&mut nodes, 10);
    dummy_time_provider.increase_by(20);
    process_messages(&mut nodes, 10);
    dummy_time_provider.increase_by(20);
    process_messages(&mut nodes, 10);

    debug_print_qpid_info_for_nodes(&nodes);
    verify_qpid_invariant(&nodes);
    ensure_only_single_root(&nodes);
}

#[test]
fn membership_regression_joins() {
    // We had a race condition if two nodes joined at the same time.
    // Some messages are truncated:
    // Race condition:
    // Node 1 joins network at node 0
    // Node 2 joins network at node 0

    // Node 1 gets 2's tree update
    // Node 1 sends update to 0
    // Node 1 gets update from 0

    // Node 1 get 1's tree update
    // Parent is set to self ERROR, this means that the parent will never be set properly, as it has already been set to something else.

    // This test should check if this is fixed.

    let settings = GeneralWaitingRoomSettings {
        ..Default::default()
    };

    log::info!("Instantiating dummy time and network");
    let dummy_time_provider = DummyTimeProvider::new();
    let dummy_random_provider = DeterministicRandomProvider::new(1);
    let dummy_network = DummyNetwork::new(dummy_time_provider.clone(), Latency::Fixed(20));

    let mut nodes = vec![];

    let node_count = 3;
    log::info!("Creating {} waitingroom nodes", node_count);
    for node_id in 0..node_count {
        let node = DistributedWaitingRoom::new(
            settings,
            node_id,
            dummy_time_provider.clone(),
            dummy_random_provider.clone(),
            dummy_network.clone(),
        );
        nodes.push(node);
    }

    // Now we initialise the node number 0 as the "first" node.
    nodes[0].initialise_alone().unwrap();

    // Now we add the other nodes, one by one.
    #[allow(clippy::needless_range_loop)]
    for i in 1..node_count {
        log::debug!("Adding node {}", i);
        // nodes[0].add_node(i).unwrap();
        nodes[i].join_at(0).unwrap();
    }
    dummy_time_provider.increase_by(20);
    process_messages(&mut nodes, 10);

    // Now, we need to make sure that node 1 gets node 2's tree update, then all other messages are processed, then node 1 gets its own tree update.
    // A kind of hacky way we can do this is to just 1's tree update message. We'll add it back in later.
    let message = dummy_network.get_messages_mut().remove(0);

    // Now we process all the other messages
    dummy_time_provider.increase_by(20);
    process_messages(&mut nodes, 10);
    dummy_time_provider.increase_by(20);
    process_messages(&mut nodes, 10);

    // Now we add the message back in
    dummy_network.get_messages_mut().push(message);

    // Now we process the message
    dummy_time_provider.increase_by(20);
    process_messages(&mut nodes, 10);
    // And now, this should succeed without any errors.

    debug_print_qpid_info_for_nodes(&nodes);
    verify_qpid_invariant(&nodes);
    ensure_only_single_root(&nodes);
}

#[test]
fn update_invariant_fail_reg() {}
