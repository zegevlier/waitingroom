use crate::{messages::NodeToNodeMessage, weight_table::Weight};
use waitingroom_core::{
    network::{Network, NetworkHandle},
    pass::Pass,
    random::RandomProvider,
    settings,
    ticket::{Ticket, TicketType},
    time::{Time, TimeProvider},
    NodeId, WaitingRoomError, WaitingRoomMessageTriggered, WaitingRoomTimerTriggered,
    WaitingRoomUserTriggered,
};
use waitingroom_local_queue::LocalQueue;
use waitingroom_spanning_trees::SpanningTree;

use crate::weight_table::WeightTable;
use settings::GeneralWaitingRoomSettings;

#[cfg(test)]
mod test;

mod count;
mod fault_detection;
mod membership_changes;
mod qpid;

// The testing module is only available when the testing feature is enabled.
#[cfg(feature = "testing")]
pub mod testing;

/// This is the waiting room implementation described in the associated thesis.
/// TODO: Add more information here.
#[derive(Debug)]
pub struct DistributedWaitingRoom<T, R, N>
where
    T: TimeProvider,
    R: RandomProvider,
    N: Network<NodeToNodeMessage>,
{
    /// The local queue is the queue on this node. It contains all the tickets that are waiting to be let in.
    local_queue: LocalQueue,
    /// The local queue leaving list is a list of tickets that are allowed to leave the queue, but have not yet done so.
    local_queue_leaving_list: Vec<Ticket>,
    /// The local on site list is a list of passes that are currently on site.
    local_on_site_list: Vec<Pass>,

    /// Settings passed in when creating the waiting room.
    settings: GeneralWaitingRoomSettings,
    /// The node ID is a unique identifier for this node.
    node_id: NodeId,

    /// The network handle is used to send and receive messages to and from other nodes.
    network_handle: N::NetworkHandle,

    /// The time provider is used to get the current time, TODO: And set timers. This is passed in to allow for deterministic testing.
    time_provider: T,
    /// The random provider is used to generate random numbers. This is passed in to allow for deterministic testing.
    random_provider: R,

    // Also see qpid.rs
    /// The QPID parent is the ID of the parent node in the QPID tree.
    qpid_parent: Option<NodeId>,
    /// The QPID weight table is a list of all the neighbours of this node, and their current "weights".
    qpid_weight_table: WeightTable,
    /// Monotonically increasing counter for the QPID update and FindRoot messages sent to each node.
    /// This is used to determine whether a message is outdated or not.
    qpid_update_iterations: Vec<(NodeId, u64)>,
    /// The last value sent in a QPID update message to this node.
    qpid_last_update_values: Vec<(NodeId, Weight)>,

    // Also see count.rs
    /// The count parent is the ID of the parent node in the count tree.
    /// The count tree is used to determine the total number of users on the site, which is then used to ensure the correct number of users are on site.
    count_parent: Option<NodeId>,
    /// Each "count" has an iteration number, which is used to determine which count is the most recent.
    count_iteration: Time,
    /// The count responses are used to store the responses from the neighbours in the count tree. They are aggregated sent to the parent when all responses are received.
    count_responses: Vec<(NodeId, usize, usize)>,
    /// The number of failed counts in a row. If this number is too high, the tree is restructured.
    failed_counts: usize,

    /// This list includes all members of the network, also the ones that are not neighbours in the QPID network.
    network_members: Vec<NodeId>,
    spanning_tree: SpanningTree,
    tree_iteration: usize,

    // fd is fault detection. This is in this file, as it is only two functions.
    /// Fault detection last check is the time of the last true check. The timer function is triggered more frequently, to detect faults faster.
    fd_last_check_time: Time,
    /// Fault detection last check node is the node that last checked.
    fd_last_check_node: Option<NodeId>,
    /// The fault detection queue contains the nodes that need to be checked. When it is empty, it gets refilled with all nodes in a random order.
    fd_queue: Vec<NodeId>,

    // TODO Write docs
    should_send_find_root: bool,
}

impl<T, R, N> WaitingRoomUserTriggered for DistributedWaitingRoom<T, R, N>
where
    T: TimeProvider,
    R: RandomProvider,
    N: Network<NodeToNodeMessage>,
{
    fn join(&mut self) -> Result<waitingroom_core::ticket::Ticket, WaitingRoomError> {
        log::info!("[NODE {}] join", self.node_id);

        if self.qpid_parent.is_none() {
            return Err(WaitingRoomError::QPIDNotInitialized);
        }
        let ticket = waitingroom_core::ticket::Ticket::new(
            self.node_id,
            self.settings.ticket_refresh_time,
            self.settings.ticket_expiry_time,
            &self.time_provider,
            &self.random_provider,
        );
        log::debug!(
            "[NODE {}] created ticket {}",
            self.node_id,
            ticket.identifier
        );
        self.enqueue(ticket)?;
        Ok(ticket)
    }

    fn check_in(
        &mut self,
        ticket: waitingroom_core::ticket::Ticket,
    ) -> Result<waitingroom_core::CheckInResponse, WaitingRoomError> {
        log::info!("[NODE {}] check in {}", self.node_id, ticket.identifier);
        if ticket.is_expired(&self.time_provider) {
            // This happens when a user has not refreshed their ticket in time.
            return Err(WaitingRoomError::TicketExpired);
        }

        if ticket.node_id != self.node_id {
            // This happens when the user tries to check in at a different node.
            // This is expected when the previous node went down. The user will need to re-join the queue at the new node.
            // Since, when we get here, the ticket is already confirmed to be valid, we can just add the ticket to the queue.
            self.enqueue(ticket)?;
        }

        // TODO: Make a better estimate of the position. A super simple way would be to multiply by the number of nodes, but that kinda sucks.
        let position_estimate = match self.local_queue.get_position(ticket.identifier) {
            Some(position) => position + 1, // 0 is reserved for users who are allowed to leave the queue right now.
            None => {
                if self.local_queue_leaving_list.contains(&ticket) {
                    // The ticket is in the queue leaving list.
                    // This means that the user can now leave the queue.
                    // When this happens, we send the user's position estimate as 0.
                    0
                } else {
                    // The ticket is not in the queue leaving list, nor is it in the queue.
                    // This usually means the ticket has already been used to leave the queue.
                    // They can't use this ticket again, so it is invalid.
                    return Err(WaitingRoomError::TicketNotInQueue);
                }
            }
        };

        let position_estimate = if position_estimate > ticket.previous_position_estimate {
            // The ticket has moved backwards in the queue.
            // This should never happen with a single node, but may happen with multiple nodes.
            // If it does, we need to send the user's old position estimate to not confuse them.
            ticket.previous_position_estimate
        } else {
            position_estimate
        };

        // Call refresh on the ticket to update the join time and expiry time.
        let ticket = self
            .local_queue
            .entry(ticket.identifier)
            .or_else(|| {
                // If it's not in the local queue but we did get here, it's in the queue leaving list.
                // So, we need to update the ticket in the queue leaving list.
                let ticket = self
                    .local_queue_leaving_list
                    .iter_mut()
                    .find(|t| t.identifier == ticket.identifier)
                    .unwrap();
                Some(ticket)
            })
            .map(|ticket| {
                *ticket = ticket.refresh(
                    position_estimate,
                    self.settings.ticket_refresh_time,
                    self.settings.ticket_expiry_time,
                    &self.time_provider,
                    self.node_id,
                );
                ticket
            })
            .unwrap();

        Ok(waitingroom_core::CheckInResponse {
            new_ticket: *ticket,
            position_estimate,
        })
    }

    fn leave(
        &mut self,
        ticket: waitingroom_core::ticket::Ticket,
    ) -> Result<waitingroom_core::pass::Pass, WaitingRoomError> {
        log::info!("[NODE {}] leave {}", self.node_id, ticket.identifier);
        if ticket.is_expired(&self.time_provider) {
            // This happens when a user has not refreshed their ticket in time.
            return Err(WaitingRoomError::TicketExpired);
        }

        if ticket.node_id != self.node_id {
            // If the user tries to leave the queue at a different node, we error.
            // They need to either check in at the correct node, or re-join the queue so they can leave at the correct node.
            return Err(WaitingRoomError::TicketAtWrongNode);
        }
        // We need the ticket from the local queue leaving list, instead of the one passed in.
        // This is because this one might have more updated information. (eg. eviction time)
        let ticket = match self.local_queue_leaving_list.iter().find(|t| **t == ticket) {
            Some(ticket) => *ticket,
            None => return Err(WaitingRoomError::TicketCannotLeaveYet),
        };

        // The user is allowed to leave the queue.
        // We remove the ticket from the queue leaving list.
        self.local_queue_leaving_list.retain(|t| t != &ticket);
        // We know the number of items removed here is always 1.
        metrics::gauge!(
            "waitingoroom.to_let_in_count",
            "node_id" => self.node_id.to_string()
        )
        .decrement(1);

        // Generate a pass for the user.
        let pass = Pass::from_ticket(ticket, self.settings.pass_expiry_time, &self.time_provider);

        // And add the pass to the users on site list.
        self.local_on_site_list.push(pass);
        metrics::gauge!(
            "waitingroom.on_site_count",
            "node_id" => self.node_id.to_string()
        )
        .increment(1);

        Ok(pass)
    }

    fn validate_and_refresh_pass(
        &mut self,
        pass: waitingroom_core::pass::Pass,
    ) -> Result<waitingroom_core::pass::Pass, WaitingRoomError> {
        log::info!("[NODE {}] pass refresh {}", self.node_id, pass.identifier);
        let now_time = self.time_provider.get_now_time();

        if pass.expiry_time < now_time {
            // The user has been inactive for too long, and their pass expired.
            return Err(WaitingRoomError::PassExpired);
        }

        if pass.node_id != self.node_id {
            // The previous node has (probably) gone down, so just to make sure we count this user as being on the site, we add them to the on site list.
            self.local_on_site_list.push(pass);
            metrics::gauge!(
                "waitingroom.on_site_count",
                "node_id" => self.node_id.to_string()
            )
            .increment(1);
        }

        let pass = self
            .local_on_site_list
            .iter_mut()
            .find(|p| p.identifier == pass.identifier)
            .map(|pass| {
                *pass = pass.refresh(
                    self.node_id,
                    self.settings.pass_expiry_time,
                    &self.time_provider,
                );
                pass
            });

        match pass {
            Some(pass) => Ok(*pass),
            // If the pass is not on the list, but it was given out at the current node, they shouldn't be on the site.
            // I don't think this should ever be able to happen, but it might if we implement kicking users from the site.
            None => Err(WaitingRoomError::PassNotInList),
        }
    }
}

impl<T, R, N> WaitingRoomTimerTriggered for DistributedWaitingRoom<T, R, N>
where
    T: TimeProvider,
    R: RandomProvider,
    N: Network<NodeToNodeMessage>,
{
    fn cleanup(&mut self) -> Result<(), WaitingRoomError> {
        log::info!("[NODE {}] cleanup", self.node_id);
        let now_time = self.time_provider.get_now_time();

        // Remove expired tickets from the local queue.
        let removed_count = self.local_queue.remove_expired(now_time);
        metrics::gauge!(
            "waitingroom.in_queue_count",
            "node_id" => self.node_id.to_string()
        )
        .decrement(removed_count as f64);

        // Remove expired passes from the on site list.
        self.local_on_site_list
            .retain(|pass| pass.expiry_time > now_time);
        metrics::gauge!(
            "waitingroom.on_site_count",
            "node_id" => self.node_id.to_string()
        )
        .set(self.local_on_site_list.len() as f64);

        // We *could* trigger dequeues here, since we know a number of people need to be let out of the queue,
        // but for simplicity we won't. Instead, we'll rely on the ensure_correct_user_count function to do this.
        // TODO(later): This could be added in the future to make the system a bit faster.

        // Remove expired tickets from the queue leaving list.
        self.local_queue_leaving_list
            .retain(|ticket| ticket.expiry_time > now_time);
        metrics::gauge!(
            "waitingroom.to_let_in_count",
            "node_id" => self.node_id.to_string()
        )
        .set(self.local_queue_leaving_list.len() as f64);

        Ok(())
    }

    fn eviction(&mut self) -> Result<(), WaitingRoomError> {
        log::info!("[NODE {}] eviction", self.node_id);
        // Only start a count if we are the QPID root node.
        if self.qpid_parent != Some(self.node_id) {
            log::debug!("[NODE {}] not root node, not starting count", self.node_id);
            return Ok(());
        }

        // We use the current time as the count iteration
        let iteration = self.time_provider.get_now_time();
        log::info!(
            "[NODE {}] starting new count it: {}",
            self.node_id,
            iteration
        );
        // This will start the count process from this node.
        self.count_request(self.node_id, iteration)
    }

    fn fault_detection(&mut self) -> Result<(), WaitingRoomError> {
        log::debug!("[NODE {}] fault detection", self.node_id);
        let now_time = self.time_provider.get_now_time();

        if self.network_members.len() <= 1 {
            // If there is only one node in the network, we don't need to do fault detection.
            return Ok(());
        }

        // If we have a last check node, and we haven't had a response after the timeout, we consider the node to be down.
        if let Some(last_check_node) = self.fd_last_check_node {
            if now_time - self.fd_last_check_time > self.settings.fault_detection_timeout {
                log::info!("[NODE {}] node {} is down", self.node_id, last_check_node);
                self.remove_node(last_check_node)?;
                // We set the last check node to None.
                self.fd_last_check_node = None;
            }
            // If it's not been too long yet, we do nothing.
        }
        // Else, if we don't have a last check node, we only check a node if it's been long enough since the last check.
        else if self.settings.fault_detection_period < now_time - self.fd_last_check_time {
            // We pick a random node to check.
            let node_to_check = self.fd_queue.pop().unwrap_or_else(|| {
                let mut nodes = self.network_members.clone();
                // Remove ourselves from the list.
                nodes.retain(|&n| n != self.node_id);
                self.random_provider.shuffle(&mut nodes);
                nodes.pop().unwrap()
            });

            log::debug!("[NODE {}] checking node {}", self.node_id, node_to_check);
            // This is the message it needs to respond to within the timeout.
            self.network_handle
                .send_message(
                    node_to_check,
                    NodeToNodeMessage::FaultDetectionRequest(now_time),
                )
                .unwrap();
            // We update the last check node and time.
            self.fd_last_check_node = Some(node_to_check);
            self.fd_last_check_time = now_time;
        }

        Ok(())
    }
}

impl<T, R, N> WaitingRoomMessageTriggered for DistributedWaitingRoom<T, R, N>
where
    T: TimeProvider,
    R: RandomProvider,
    N: Network<NodeToNodeMessage>,
{
    fn receive_message(&mut self) -> Result<bool, WaitingRoomError> {
        // This function only redirects the messages to the correct handler.
        if let Some(message) = self.network_handle.receive_message()? {
            match message.message {
                NodeToNodeMessage::QPIDUpdateMessage {
                    weight,
                    updated_iteration,
                } => self.qpid_handle_update(message.from_node, weight, updated_iteration),
                NodeToNodeMessage::QPIDDeleteMin => self.qpid_delete_min(),
                NodeToNodeMessage::QPIDFindRootMessage {
                    weight,
                    last_eviction,
                    updated_iteration,
                } => self.qpid_handle_find_root(
                    message.from_node,
                    weight,
                    last_eviction,
                    updated_iteration,
                ),
                NodeToNodeMessage::CountRequest(count_iteration) => {
                    self.count_request(message.from_node, count_iteration)
                }
                NodeToNodeMessage::CountResponse {
                    iteration,
                    queue_count,
                    on_site_count,
                } => self.count_response(message.from_node, iteration, queue_count, on_site_count),
                NodeToNodeMessage::FaultDetectionRequest(check_id) => {
                    self.fault_detection_request(message.from_node, check_id)
                }
                NodeToNodeMessage::FaultDetectionResponse(check_id) => {
                    self.fault_detection_response(message.from_node, check_id)
                }
                NodeToNodeMessage::NodeAdded(node_id, spanning_tree, spanning_tree_iteration) => {
                    self.node_add_message(node_id, spanning_tree, spanning_tree_iteration)
                }
                NodeToNodeMessage::NodeRemoved(node_id, spanning_tree, spanning_tree_iteration) => {
                    self.node_remove_message(node_id, spanning_tree, spanning_tree_iteration)
                }
                NodeToNodeMessage::TreeRestructure(spanning_tree, spanning_tree_iteration) => {
                    self.restructure_tree_message(spanning_tree, spanning_tree_iteration)
                }
                NodeToNodeMessage::NodeJoin(node_id) => self.node_join_message(node_id),
            }?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl<T, R, N> DistributedWaitingRoom<T, R, N>
where
    T: TimeProvider,
    R: RandomProvider,
    N: Network<NodeToNodeMessage>,
{
    pub fn new(
        settings: GeneralWaitingRoomSettings,
        node_id: NodeId,
        time_provider: T,
        random_provider: R,
        network: N,
    ) -> Self {
        let network_handle = match network.join(node_id) {
            Ok(handle) => handle,
            Err(err) => {
                panic!("Failed to join network: {:?}", err);
            }
        };

        Self {
            node_id,
            time_provider,
            random_provider,
            settings,
            network_handle,
            qpid_weight_table: WeightTable::new(node_id),
            network_members: vec![node_id],
            spanning_tree: SpanningTree::from_member_list(vec![node_id]), // Since we usually just join an existing network, we start with an empty tree.
            tree_iteration: 0, // Always 0 until we receive the first tree from another node.
            local_queue: LocalQueue::new(),
            local_on_site_list: vec![],
            local_queue_leaving_list: vec![],
            count_responses: vec![],
            fd_queue: vec![],
            qpid_update_iterations: vec![],
            count_iteration: Time::MIN,
            fd_last_check_time: Time::MIN,
            count_parent: None,
            fd_last_check_node: None,
            qpid_parent: None,
            should_send_find_root: false,
            qpid_last_update_values: vec![],
            failed_counts: 0,
        }
    }

    /// DO NOT CALL - Temporary testing function to overwrite the QPID parent and weight table.
    /// This will be removed once recovery is implemented (since that's basically the same system).
    pub fn testing_overwrite_qpid(
        &mut self,
        parent: Option<NodeId>,
        weight_table: Vec<(NodeId, Weight)>,
    ) {
        self.qpid_parent = parent;
        self.qpid_weight_table = WeightTable::from_vec(self.node_id, weight_table);
        self.network_members = self.qpid_weight_table.get_all_neighbours();
    }

    /// Add a ticket to the local queue, incrementing the metric if the ticket type is normal.
    fn enqueue(&mut self, ticket: Ticket) -> Result<(), WaitingRoomError> {
        self.local_queue.enqueue(ticket);
        if ticket.ticket_type == TicketType::Normal {
            metrics::gauge!(
                "waitingroom.in_queue_count",
                "node_id" => self.node_id.to_string()
            )
            .increment(1);
        }
        // We only call QPID insert if the current join time is less than the current QPID weight.
        // This means that all inserts that are *not* at the front of the queue don't make any QPID messages, which is nice.
        let new_weight = Weight::new(ticket.join_time, ticket.identifier, self.node_id);
        if new_weight < self.qpid_weight_table.get_weight(self.node_id).unwrap() {
            self.qpid_insert(new_weight)?;
        }
        Ok(())
    }

    /// Remove the element at the front of the local queue, decrementing the metric if the ticket type is normal.
    fn dequeue(&mut self) -> Option<Ticket> {
        let element = self.local_queue.dequeue();
        if element.is_some() && element.as_ref().unwrap().ticket_type == TicketType::Normal {
            metrics::gauge!(
                "waitingroom.in_queue_count",
                "node_id" => self.node_id.to_string()
            )
            .decrement(1);
        }
        element
    }

    /// This function triggers an amount of QPID dequeue operations. The amount is the waiting room's minimum user count minus the current user count, provided in the parameter.
    /// If there are too many users on the site, this function will add dummy users to the queue, which will be dequeued by the QPID algorithm and thus lower the user count on the site.
    fn ensure_correct_site_count(
        &mut self,
        queue_count: usize,
        on_site_count: usize,
    ) -> Result<(), WaitingRoomError> {
        log::info!("[NODE {}] let users out of queue", self.node_id);
        if on_site_count < self.settings.target_user_count {
            let to_let_out = queue_count.min(self.settings.target_user_count - on_site_count);
            log::debug!(
                "[NODE {}] not enough users on site, need to let {} users out of queue",
                self.node_id,
                to_let_out
            );
            // There is no need to let out more users than there are in the queue.
            for _ in 0..to_let_out {
                self.qpid_delete_min()?;
            }
        }

        Ok(())
    }
}
