use waitingroom_core::{
    network::{DummyNetwork, Latency},
    random::DeterministicRandomProvider,
    settings::GeneralWaitingRoomSettings,
    time::{DummyTimeProvider, Time},
    NodeId, WaitingRoomMessageTriggered, WaitingRoomTimerTriggered, WaitingRoomUserTriggered,
};

use test_log::test;

use crate::{messages::NodeToNodeMessage, DistributedWaitingRoom};

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

    node.testing_overwrite_qpid(Some(1), vec![(1, Time::MAX)]);

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
    let init_weight_table: Vec<(NodeId, Time)> = (0..node_count).map(|v| (v, Time::MAX)).collect();
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

    process_messages(&mut nodes);

    dummy_time_provider.increase_by(10);

    let ticket2 = nodes[1].join().unwrap();
    process_messages(&mut nodes);

    nodes[1].qpid_delete_min().unwrap();
    process_messages(&mut nodes);

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
    let init_weight_table: Vec<(NodeId, Time)> = (0..node_count).map(|v| (v, Time::MAX)).collect();
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

    process_messages(&mut nodes);

    dummy_time_provider.increase_by(10);

    let ticket2 = nodes[1].join().unwrap();

    process_messages(&mut nodes);

    nodes.iter_mut().for_each(|node| {
        node.ensure_correct_user_count().unwrap();
    });

    process_messages(&mut nodes);

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
        node.ensure_correct_user_count().unwrap();
    });

    process_messages(&mut nodes);

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
        node.ensure_correct_user_count().unwrap();
    });

    process_messages(&mut nodes);

    let checkin_result2 = nodes[new_ticket.node_id].check_in(new_ticket).unwrap();

    assert!(checkin_result2.position_estimate == 0);

    log::info!("Done!");
}

fn process_messages(nodes: &mut [Node]) {
    log::debug!("Processing messages");
    while nodes.iter_mut().any(|n| n.receive_message().unwrap()) {}
}

#[test]
fn simple_fault_test() {
    let settings = GeneralWaitingRoomSettings {
        fault_detection_interval: 1000,
        fault_detection_timeout: 199,
        fault_detection_period: 100,
        ..Default::default()
    };

    log::info!("Instantiating dummy time and network");
    let dummy_time_provider = DummyTimeProvider::new();
    let dummy_random_provider = DeterministicRandomProvider::new(1);
    let dummy_network = DummyNetwork::new(dummy_time_provider.clone(), Latency::Fixed(20)); // 20ms latency

    let mut nodes = vec![];

    let node_count = 2;
    log::info!("Creating {} waitingroom nodes", node_count);
    let init_weight_table: Vec<(NodeId, Time)> = (0..node_count).map(|v| (v, Time::MAX)).collect();
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

    assert!(dummy_network.total_messages_in_network() == 2);

    dummy_time_provider.increase_by(20);

    process_messages(&mut nodes);

    assert!(
        dummy_network.total_messages_in_network() == 2,
        "They should both have replied."
    );

    dummy_time_provider.increase_by(20);
    process_messages(&mut nodes);

    assert!(
        dummy_network.total_messages_in_network() == 0,
        "the messages should be processed"
    );

    // These are before the interval, so no messages should be sent
    nodes[0].fault_detection().unwrap();
    nodes[1].fault_detection().unwrap();

    assert!(
        dummy_network.total_messages_in_network() == 0,
        "No messages should be sent"
    );

    // Now, we wait for the interval to pass
    dummy_time_provider.increase_by(1000);
    // And we make node 1 "fail" (we remove it from the nodes list so it can't send messages)
    nodes.remove(1);

    // Now do a fault check on 0, which should detect that 1 is not responding
    nodes[0].fault_detection().unwrap();

    process_messages(&mut nodes); // This doesn't do anything, because node 1 is not in the network anymore
    assert!(
        dummy_network.total_messages_in_network() == 1,
        "The old message shouldn't be picked up"
    );

    // Now we wait before the timeout
    dummy_time_provider.increase_by(100);

    nodes[0].fault_detection().unwrap(); // This is before the timeout, so it should not do anything

    process_messages(&mut nodes);

    // The old message will still be there, since it's not picked up by the other node, but no new messages should be sent
    assert!(
        dummy_network.total_messages_in_network() == 1,
        "No new messages should be sent"
    );

    // Now we wait for the timeout
    dummy_time_provider.increase_by(100);
    nodes[0].fault_detection().unwrap(); // This is after the timeout, so node 0 should consider node 1 faulty.

    // TODO: Actually check this. This is currently not possible to check, but will be once I implement fault recovery.
    // However, we can see that the console logs "node 1 is down", so the fault detection works, at least in this test case.

    log::info!("Done");
}
