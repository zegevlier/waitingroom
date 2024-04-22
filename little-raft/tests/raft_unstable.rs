use bytes::Bytes;
use crossbeam_channel as channel;
use crossbeam_channel::{unbounded, Receiver, Sender};
use rand::{thread_rng, Rng};
use rand::seq::SliceRandom;
use little_raft::{
    cluster::Cluster,
    message::Message,
    replica::Replica,
    state_machine::{Snapshot, StateMachine, StateMachineTransition, TransitionState},
};
use std::convert::TryInto;
use std::sync::{Arc, Mutex};

use std::{collections::BTreeMap, thread, time::Duration};

const HEARTBEAT_TIMEOUT: Duration = Duration::from_millis(50);
const MIN_ELECTION_TIMEOUT: Duration = Duration::from_millis(750);
const MAX_ELECTION_TIMEOUT: Duration = Duration::from_millis(950);

// Our state machine will carry out simple plus and minus operations on a
// number, starting from zero.
#[derive(Clone, Debug)]
struct ArithmeticOperation {
    id: usize,
    delta: i32,
}

impl StateMachineTransition for ArithmeticOperation {
    type TransitionID = usize;
    fn get_id(&self) -> Self::TransitionID {
        self.id
    }
}

// The Calculator is the state machine that maintains a number that we can add
// to or subtract from. ID is simply for convenience.
struct Calculator {
    id: usize,
    value: i32,
    applied_ids_tx: Sender<(usize, usize)>,
    pending_transitions: Vec<ArithmeticOperation>,
}

impl StateMachine<ArithmeticOperation, Bytes> for Calculator {
    fn apply_transition(&mut self, transition: ArithmeticOperation) {
        self.value += transition.delta;
        println!("id {} my value is now {} after applying delta {}", self.id, self.value, transition.delta);
    }

    fn register_transition_state(
        &mut self,
        transition_id: <ArithmeticOperation as StateMachineTransition>::TransitionID,
        state: TransitionState,
    ) {
        // Send IDs of applied transitions down the channel so we can confirm
        // they were applied in the right order.
        if state == TransitionState::Applied {
            self.applied_ids_tx
                .send((self.id, transition_id))
                .expect("could not send applied transition id");
        }
    }

    fn get_pending_transitions(&mut self) -> Vec<ArithmeticOperation> {
        let cur = self.pending_transitions.clone();
        self.pending_transitions = Vec::new();
        cur
    }

    fn get_snapshot(&mut self) -> Option<Snapshot<Bytes>> {
        println!("id {} checked for snapshot", self.id);
        None
    }

    fn create_snapshot(&mut self, index: usize, term: usize) -> Snapshot<Bytes> {
        println!("id {} created snapshot", self.id);
        Snapshot {
            last_included_index: index,
            last_included_term: term,
            data: Bytes::from(self.value.to_be_bytes().to_vec()),
        }
    }

    fn set_snapshot(&mut self, snapshot: Snapshot<Bytes>) {
        let v: Vec<u8> = snapshot.data.into_iter().collect();
        self.value = i32::from_be_bytes(v[..].try_into().expect("incorrect length"));
        println!("id {} my value is now {} after loading", self.id, self.value);
    }
}

// Our test replicas will be running each in its own thread.
struct ThreadCluster {
    id: usize,
    is_leader: bool,
    transmitters: BTreeMap<usize, Sender<Message<ArithmeticOperation, Bytes>>>,
    pending_messages: Vec<Message<ArithmeticOperation, Bytes>>,
    halt: bool,
}

impl Cluster<ArithmeticOperation, Bytes> for ThreadCluster {
    fn register_leader(&mut self, leader_id: Option<usize>) {
        if let Some(id) = leader_id {
            if id == self.id {
                self.is_leader = true;
            } else {
                self.is_leader = false;
            }
        } else {
            self.is_leader = false;
        }
    }

    fn send_message(&mut self, to_id: usize, message: Message<ArithmeticOperation, Bytes>) {
        // Drop messages with probability 0.25.
        let n: u8 = rand::thread_rng().gen();
        if n % 4 == 0 {
            return
        }

        if let Some(transmitter) = self.transmitters.get(&to_id) {
            transmitter.send(message).expect("could not send message");
        }
    }

    fn halt(&self) -> bool {
        self.halt
    }

    fn receive_messages(&mut self) -> Vec<Message<ArithmeticOperation, Bytes>> {
        let mut cur = self.pending_messages.clone();
        // Shuffle messages.
        cur.shuffle(&mut thread_rng());
        self.pending_messages = Vec::new();
        cur
    }
}

// Create n clusters, each with their own copy of trasmitters used for
// communication between replicas (threads).
fn create_clusters(
    n: usize,
    transmitters: BTreeMap<usize, Sender<Message<ArithmeticOperation, Bytes>>>,
) -> Vec<Arc<Mutex<ThreadCluster>>> {
    let mut clusters = Vec::new();
    for i in 0..n {
        let cluster = Arc::new(Mutex::new(ThreadCluster {
            id: i,
            is_leader: false,
            transmitters: transmitters.clone(),
            pending_messages: Vec::new(),
            halt: false,
        }));

        clusters.push(cluster);
    }

    clusters
}

// Create channels for the threads to communicate with.
fn create_communication_between_clusters(
    n: usize,
) -> (
    BTreeMap<usize, Sender<Message<ArithmeticOperation, Bytes>>>,
    Vec<Receiver<Message<ArithmeticOperation, Bytes>>>,
) {
    let (mut transmitters, mut receivers) = (BTreeMap::new(), Vec::new());
    for i in 0..n {
        let (tx, rx) = unbounded::<Message<ArithmeticOperation, Bytes>>();
        transmitters.insert(i, tx);
        receivers.push(rx);
    }

    (transmitters, receivers)
}

fn create_peer_ids(n: usize) -> Vec<Vec<usize>> {
    let mut all_peer_ids = Vec::new();
    for i in 0..n {
        let mut peer_ids = Vec::new();
        for n in 0..n {
            if n != i {
                peer_ids.push(n);
            }
        }
        all_peer_ids.push(peer_ids);
    }

    all_peer_ids
}

// Create state machines, each with its own copy on which to send
// (state_machine_id, transition_id) for transitions that have been applied.
fn create_state_machines(
    n: usize,
    applied_transitions_tx: Sender<(usize, usize)>,
) -> Vec<Arc<Mutex<Calculator>>> {
    let mut state_machines = Vec::new();
    for i in 0..n {
        let state_machine = Arc::new(Mutex::new(Calculator {
            id: i,
            value: 0,
            pending_transitions: Vec::new(),
            applied_ids_tx: applied_transitions_tx.clone(),
        }));
        state_machines.push(state_machine);
    }
    state_machines
}

// Create sending ends of message notifiers, sending ends of transition
// notifiers, receiving ends of message notifiers, receiving neds of transition
// notifiers.
fn create_notifiers(
    n: usize,
) -> (
    Vec<Sender<()>>,
    Vec<Sender<()>>,
    Vec<Receiver<()>>,
    Vec<Receiver<()>>,
) {
    let mut message_tx = Vec::new();
    let mut message_rx = Vec::new();
    let mut transition_tx = Vec::new();
    let mut transition_rx = Vec::new();
    for _ in 0..n {
        let (message_notifier_tx, message_notifier_rx) = channel::unbounded();
        let (transition_notifier_tx, transition_notifier_rx) = channel::unbounded();
        message_tx.push(message_notifier_tx);
        message_rx.push(message_notifier_rx);
        transition_tx.push(transition_notifier_tx);
        transition_rx.push(transition_notifier_rx);
    }

    (message_tx, transition_tx, message_rx, transition_rx)
}

fn run_clusters_communication(
    mut clusters: Vec<Arc<Mutex<ThreadCluster>>>,
    mut cluster_message_receivers: Vec<Receiver<Message<ArithmeticOperation, Bytes>>>,
    mut message_notifiers_tx: Vec<Sender<()>>,
) {
    for _ in (0..clusters.len()).rev() {
        let cluster = clusters.pop().unwrap();
        let cluster_message_rx = cluster_message_receivers.pop().unwrap();
        let message_notifier = message_notifiers_tx.pop().unwrap();

        // For each cluster, start a thread where we notify the cluster replica
        // of a new message as soon as we receive one for it.
        thread::spawn(move || loop {
            let msg = cluster_message_rx.recv().unwrap();
            match cluster.lock() {
                Ok(mut unlocked_cluster) => {
                    unlocked_cluster.pending_messages.push(msg);
                    message_notifier
                        .send(())
                        .expect("could not notify of message");
                }
                _ => return,
            }
        });
    }
}

fn run_arithmetic_operation_on_cluster(
    clusters: Vec<Arc<Mutex<ThreadCluster>>>,
    state_machines: Vec<Arc<Mutex<Calculator>>>,
    transition_notifiers: Vec<Sender<()>>,
    delta: i32,
    id: usize,
) {
    // Sleep longer because in this test we're dropping 25% of all messages.
    thread::sleep(Duration::from_secs(2));
    // Find the leader and send the transition request to it.
    for cluster in clusters.iter() {
        let cluster = cluster.lock().unwrap();
        if cluster.is_leader {
            state_machines[cluster.id]
                .lock()
                .unwrap()
                .pending_transitions
                .push(ArithmeticOperation { delta, id });
            transition_notifiers[cluster.id]
                .send(())
                .expect("could not send transition notification");
            break;
        }
    }

    // Sleep long.
    thread::sleep(Duration::from_secs(3));
}

fn halt_clusters(clusters: Vec<Arc<Mutex<ThreadCluster>>>) {
    thread::sleep(Duration::from_secs(1));
    for cluster in clusters.iter() {
        let mut c = cluster.lock().unwrap();
        c.halt = true;
    }
    thread::sleep(Duration::from_secs(2));
}

#[test]
fn run_replicas() {
    let n = 3;
    // We are going to test that three replicas can elect a leader and process a
    // few simple operations.
    //
    // Main complexity of this test set up comes from the fact that everything
    // is running on a single machine, so we have to keep track of every
    // cluster, replica, and state machine object. In the real world usage of
    // the library it's unlikely there will ever be more than a single instance
    // of each object per process or even a physical machine.
    let (transmitters, receivers) = create_communication_between_clusters(3);
    let clusters = create_clusters(n, transmitters);
    let peer_ids = create_peer_ids(n);
    let noop = ArithmeticOperation { delta: 0, id: 0 };
    let (applied_transitions_tx, _applied_transitions_rx) = unbounded();
    let state_machines = create_state_machines(n, applied_transitions_tx);
    let (message_tx, transition_tx, message_rx, transition_rx) = create_notifiers(n);
    for i in 0..n {
        let noop = noop.clone();
        let local_peer_ids = peer_ids[i].clone();
        let cluster = clusters[i].clone();
        let state_machine = state_machines[i].clone();
        let m_rx = message_rx[i].clone();
        let t_rx = transition_rx[i].clone();
        thread::spawn(move || {
            let mut replica = Replica::new(
                i,
                local_peer_ids,
                cluster,
                state_machine,
                1,
                noop.clone(),
                HEARTBEAT_TIMEOUT,
                (MIN_ELECTION_TIMEOUT, MAX_ELECTION_TIMEOUT),
            );

            replica.start(m_rx, t_rx);
        });
    }

    run_clusters_communication(clusters.clone(), receivers, message_tx);
    run_arithmetic_operation_on_cluster(
        clusters.clone(),
        state_machines.clone(),
        transition_tx.clone(),
        5,
        1,
    );

    // In this test, we confirm that the cluster converged on true value one by
    // one after each arithmetic operation. This is different from
    // raft_stable.rs, where we check the order in which transition have been
    // applied post-factum. We can't do the same in raft_unstable.rs, because
    // replicas reload from snapshots in this test, meaning not all replicas go
    // over all transitions. Some replicas load directly from their peer's
    // snapshots.
    for machine in state_machines.clone() {
        assert_eq!(machine.lock().unwrap().value, 5);
    }

    run_arithmetic_operation_on_cluster(
        clusters.clone(),
        state_machines.clone(),
        transition_tx.clone(),
        -51,
        2,
    );

    for machine in state_machines.clone() {
        assert_eq!(machine.lock().unwrap().value, -46);
    }

    run_arithmetic_operation_on_cluster(
        clusters.clone(),
        state_machines.clone(),
        transition_tx.clone(),
        -511,
        3,
    );


    for machine in state_machines.clone() {
        assert_eq!(machine.lock().unwrap().value, -557);
    }

    run_arithmetic_operation_on_cluster(clusters.clone(), state_machines.clone(), transition_tx.clone(), 3, 4);

    for machine in state_machines.clone() {
        assert_eq!(machine.lock().unwrap().value, -554);
    }

    halt_clusters(clusters);
}
