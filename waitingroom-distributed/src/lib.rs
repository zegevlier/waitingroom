use messages::NodeToNodeMessage;
use waitingroom_core::{
    metrics,
    network::{Network, NetworkHandle},
    pass::Pass,
    random::RandomProvider,
    retain_with_count, settings,
    ticket::{Ticket, TicketType},
    time::{Time, TimeProvider},
    NodeId, WaitingRoomError, WaitingRoomMessageTriggered, WaitingRoomTimerTriggered,
    WaitingRoomUserTriggered,
};
use waitingroom_local_queue::LocalQueue;

pub use settings::GeneralWaitingRoomSettings;
use weight_table::WeightTable;

#[cfg(test)]
mod test;

mod weight_table;

pub mod messages;

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

    /// The QPID parent is the ID of the parent node in the QPID tree.
    qpid_parent: Option<NodeId>,
    /// The QPID weight table is a list of all the neighbours of this node, and their current "weights".
    qpid_weight_table: WeightTable,

    /// The count parent is the ID of the parent node in the count tree.
    /// The count tree is used to determine the total number of users on the site, which is then used to ensure the correct number of users are on site.
    count_parent: Option<NodeId>,
    /// Each "count" has an iteration number, which is used to determine which count is the most recent.
    count_iteration: Time,
    /// The count responses are used to store the responses from the neighbours in the count tree. They are aggregated sent to the parent when all responses are received.
    count_responses: Vec<(NodeId, usize)>,
}

impl<T, R, N> WaitingRoomUserTriggered for DistributedWaitingRoom<T, R, N>
where
    T: TimeProvider,
    R: RandomProvider,
    N: Network<NodeToNodeMessage>,
{
    fn join(&mut self) -> Result<waitingroom_core::ticket::Ticket, WaitingRoomError> {
        log::info!("[NODE {}] join", self.node_id);
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
        self.enqueue(ticket);
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
            self.enqueue(ticket);
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

        if !self.local_queue_leaving_list.contains(&ticket) {
            // The user is not allowed to leave the queue yet.
            return Err(WaitingRoomError::TicketCannotLeaveYet);
        }

        // The user is allowed to leave the queue.
        // We remove the ticket from the queue leaving list.
        self.local_queue_leaving_list.retain(|t| t != &ticket);
        // We know the number of items removed here is always 1.
        metrics::waitingroom::to_be_let_in_count(self.node_id).dec();

        // Generate a pass for the user.
        let pass = Pass::from_ticket(ticket, self.settings.pass_expiry_time, &self.time_provider);

        // And add the pass to the users on site list.
        self.local_on_site_list.push(pass);
        metrics::waitingroom::on_site_count(self.node_id).inc();

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
            metrics::waitingroom::on_site_count(self.node_id).inc();
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
        metrics::waitingroom::in_queue_count(self.node_id).dec_by(removed_count);

        // Remove expired passes from the on site list.
        let removed_count = retain_with_count(&mut self.local_on_site_list, |pass| {
            pass.expiry_time > now_time
        });
        metrics::waitingroom::on_site_count(self.node_id).dec_by(removed_count);

        // We *could* trigger dequeues here, since we know a number of people need to be let out of the queue,
        // but for simplicity we won't. Instead, we'll rely on the ensure_correct_user_count function to do this.
        // TODO(later): This could be added in the future to make the system a bit faster.

        // Remove expired tickets from the queue leaving list.
        let removed_count = retain_with_count(&mut self.local_queue_leaving_list, |ticket| {
            ticket.expiry_time > now_time
        });
        metrics::waitingroom::to_be_let_in_count(self.node_id).dec_by(removed_count);

        Ok(())
    }

    fn ensure_correct_user_count(&mut self) -> Result<(), WaitingRoomError> {
        log::info!("[NODE {}] ensure correct user count", self.node_id);
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
                NodeToNodeMessage::QPIDUpdateMessage(weight) => {
                    self.qpid_handle_update(message.from_node, weight)
                }
                NodeToNodeMessage::QPIDDeleteMin => self.qpid_delete_min(),
                NodeToNodeMessage::QPIDFindRootMessage(weight) => {
                    self.qpid_handle_find_root(message.from_node, weight)
                }
                NodeToNodeMessage::CountRequest(count_iteration) => {
                    self.count_request(message.from_node, count_iteration)
                }
                NodeToNodeMessage::CountResponse(count_iteration, count) => {
                    self.count_response(message.from_node, count_iteration, count)
                }
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
            local_queue: LocalQueue::new(),
            local_queue_leaving_list: vec![],
            local_on_site_list: vec![],
            qpid_parent: None,
            qpid_weight_table: WeightTable::new(),
            node_id,
            time_provider,
            random_provider,
            settings,
            network_handle,
            count_parent: None,
            count_iteration: Time::MIN,
            count_responses: vec![],
        }
    }

    /// DO NOT CALL - Temporary testing function to overwrite the QPID parent and weight table.
    /// This will be removed once recovery is implemented (since that's basically the same system).
    pub fn testing_overwrite_qpid(
        &mut self,
        parent: Option<NodeId>,
        weight_table: Vec<(NodeId, Time)>,
    ) {
        self.qpid_parent = parent;
        self.qpid_weight_table = WeightTable::from_vec(weight_table);
    }

    /// Add a ticket to the local queue, incrementing the metric if the ticket type is normal.
    fn enqueue(&mut self, ticket: Ticket) {
        self.local_queue.enqueue(ticket);
        if ticket.ticket_type == TicketType::Normal {
            metrics::waitingroom::in_queue_count(self.node_id).inc();
        }
        // We only call QPID insert if the current join time is less than the current QPID weight.
        // This means that all inserts that are *not* at the front of the queue don't make any QPID messages, which is nice.
        if ticket.join_time < self.qpid_weight_table.get(self.node_id).unwrap() {
            self.qpid_insert(ticket.join_time);
        }
    }

    /// Remove the element at the front of the local queue, decrementing the metric if the ticket type is normal.
    fn dequeue(&mut self) -> Option<Ticket> {
        let element = self.local_queue.dequeue();
        if element.is_some() && element.as_ref().unwrap().ticket_type == TicketType::Normal {
            metrics::waitingroom::in_queue_count(self.node_id).dec();
        }
        element
    }

    /// For this, and all other QPID functions, see QPID paper and thesis for more information.
    /// Algorithm 1 - insert
    fn qpid_insert(&mut self, weight: Time) {
        self.qpid_weight_table.set(self.node_id, weight);
        if let Some(qpid_parent) = self.qpid_parent {
            if qpid_parent == self.node_id {
                return;
            }
            let updated_weight = self.qpid_weight_table.compute_weight(qpid_parent);
            if updated_weight != self.qpid_weight_table.get(qpid_parent).unwrap() {
                self.qpid_weight_table.set(qpid_parent, updated_weight);
                self.network_handle
                    .send_message(
                        qpid_parent,
                        NodeToNodeMessage::QPIDUpdateMessage(updated_weight),
                    )
                    .unwrap();
            }
        } else {
            log::warn!("QPID parent is None when trying to insert");
        }
    }

    /// For this, and all other QPID functions, see QPID paper and thesis for more information.
    /// Algorithm 2 - update
    fn qpid_handle_update(
        &mut self,
        from_node: NodeId,
        weight: Time,
    ) -> Result<(), WaitingRoomError> {
        log::info!("[NODE {}] handle update", self.node_id);
        if self.qpid_parent.is_none() {
            return Err(WaitingRoomError::QPIDNotInitialized);
        }
        let qpid_parent = self.qpid_parent.unwrap();

        self.qpid_weight_table.set(from_node, weight);
        if self.qpid_parent == Some(self.node_id) {
            if weight < self.qpid_weight_table.get(self.node_id).unwrap() {
                self.qpid_parent = Some(from_node);
                let updated_weight = self.qpid_weight_table.compute_weight(from_node);
                self.qpid_weight_table.set(from_node, updated_weight);
                self.network_handle
                    .send_message(
                        from_node,
                        NodeToNodeMessage::QPIDFindRootMessage(updated_weight),
                    )
                    .unwrap()
            }
        } else {
            let updated_weight = self.qpid_weight_table.compute_weight(qpid_parent);
            if updated_weight != self.qpid_weight_table.get(qpid_parent).unwrap() {
                self.qpid_weight_table.set(qpid_parent, updated_weight);
                self.network_handle
                    .send_message(
                        qpid_parent,
                        NodeToNodeMessage::QPIDUpdateMessage(updated_weight),
                    )
                    .unwrap()
            }
        }

        Ok(())
    }

    /// For this, and all other QPID functions, see QPID paper and thesis for more information.
    /// Algorithm 3 - deleteMin
    fn qpid_delete_min(&mut self) -> Result<(), WaitingRoomError> {
        log::info!("[NODE {}] QPID delete min", self.node_id);
        if self.qpid_parent.is_none() {
            return Err(WaitingRoomError::QPIDNotInitialized);
        }
        let qpid_parent = self.qpid_parent.unwrap();

        if qpid_parent != self.node_id {
            self.network_handle
                .send_message(qpid_parent, NodeToNodeMessage::QPIDDeleteMin)
                .unwrap();
            return Ok(());
        }

        if self.local_queue.is_empty() {
            return Ok(());
        }

        let ticket = self.dequeue().unwrap();

        // Update current QPID weight
        match self.local_queue.peek() {
            Some(next_ticket) => {
                self.qpid_weight_table
                    .set(self.node_id, next_ticket.join_time);
            }
            None => {
                self.qpid_weight_table.set(self.node_id, Time::MAX);
            }
        }

        if self.qpid_weight_table.any_not_max() {
            let new_parent = self.qpid_weight_table.get_smallest().unwrap();
            self.qpid_parent = Some(new_parent);
            if new_parent != self.node_id {
                let updated_weight = self.qpid_weight_table.compute_weight(new_parent);
                self.qpid_weight_table.set(new_parent, updated_weight);
                self.network_handle
                    .send_message(
                        new_parent,
                        NodeToNodeMessage::QPIDFindRootMessage(updated_weight),
                    )
                    .unwrap();
            }
        }

        match ticket.ticket_type {
            TicketType::Normal => {
                self.local_queue_leaving_list.push(ticket);
                metrics::waitingroom::to_be_let_in_count(self.node_id).inc();
            }
            TicketType::Drain => {
                // This ticket is a dummy ticket. We shouldn't do anything with it.
            }
            TicketType::Skip => {
                // For this ticket, we need to take someone else out of the queue.
                self.qpid_delete_min()?;
            }
        }

        Ok(())
    }

    /// For this, and all other QPID functions, see QPID paper and thesis for more information.
    /// Algorithm 4 - findRoot
    fn qpid_handle_find_root(
        &mut self,
        from_node: NodeId,
        weight: Time,
    ) -> Result<(), WaitingRoomError> {
        log::info!("[NODE {}] handle find root", self.node_id);

        if self.qpid_parent.is_none() {
            return Err(WaitingRoomError::QPIDNotInitialized);
        }

        let qpid_parent = self.qpid_parent.unwrap();
        self.qpid_weight_table.set(from_node, weight);
        self.qpid_parent = self.qpid_weight_table.get_smallest();
        if qpid_parent != self.node_id {
            let updated_weight = self.qpid_weight_table.compute_weight(qpid_parent);
            self.qpid_weight_table.set(qpid_parent, updated_weight);
            self.network_handle
                .send_message(
                    from_node,
                    NodeToNodeMessage::QPIDUpdateMessage(updated_weight),
                )
                .unwrap()
        }
        Ok(())
    }

    /// The count operations are used to determine the total number of users on the site on the entire network.
    /// This initiates a count request, which is then propagated through the network.
    /// See thesis for more information.
    fn count_request(
        &mut self,
        from_node: NodeId,
        count_iteration: Time,
    ) -> Result<(), WaitingRoomError> {
        log::info!("[NODE {}] count request", self.node_id);
        if count_iteration <= self.count_iteration {
            // We've already participated in a count iteration that is higher than this one.
            // We don't need to respond.
            log::debug!(
                "[NODE {}] count request from {} with count {} is lower than current count {}",
                self.node_id,
                from_node,
                count_iteration,
                self.count_iteration
            );
            return Ok(());
        }

        self.count_iteration = count_iteration;
        self.count_parent = Some(from_node);
        self.count_responses.clear();

        // If we have any neighbours, we need to ask them to participate in the count before we can respond.
        if self.qpid_weight_table.neighbour_count() > 1 || self.node_id == from_node {
            for node_id in &self.qpid_weight_table.all_neighbours() {
                if *node_id != from_node {
                    self.network_handle
                        .send_message(*node_id, NodeToNodeMessage::CountRequest(count_iteration))?;
                }
            }
        } else {
            // If we don't have any neighbours, we can respond immediately.
            if self.node_id == from_node {
                self.count_response(from_node, count_iteration, 0)?;
            } else {
                self.network_handle.send_message(
                    from_node,
                    NodeToNodeMessage::CountResponse(count_iteration, self.get_on_site_count()),
                )?;
            }
        }

        Ok(())
    }

    /// See thesis for more information.
    fn count_response(
        &mut self,
        from_node: NodeId,
        count_iteration: Time,
        count: usize,
    ) -> Result<(), WaitingRoomError> {
        log::info!(
            "[NODE {}] count response fr: {} it: {} c: {}",
            self.node_id,
            from_node,
            count_iteration,
            count
        );
        if count_iteration != self.count_iteration {
            // This message isn't part of the current count iteration. Ignore it.
            log::debug!(
                "[NODE {}] count response from {} with count {} is not for current count {}",
                self.node_id,
                from_node,
                count_iteration,
                self.count_iteration
            );
            return Ok(());
        }

        self.count_responses.push((from_node, count));

        if self.count_responses.len() >= self.qpid_weight_table.neighbour_count() {
            // We have received all responses.
            let others_count = self
                .count_responses
                .iter()
                .map(|(_, count)| *count)
                .sum::<usize>();

            let own_count = self.get_on_site_count();
            let total_count = others_count + own_count;

            if Some(self.node_id) == self.count_parent {
                // We are the count root, so we need to let users out of the queue.
                log::debug!(
                    "[NODE {}] count root with total count {}",
                    self.node_id,
                    total_count
                );
                self.ensure_correct_site_count(total_count)?;
            } else {
                // We are not the count parent node, so we need to send our total count to the parent node.
                self.network_handle.send_message(
                    self.count_parent.unwrap(),
                    NodeToNodeMessage::CountResponse(count_iteration, total_count),
                )?;
            }
        }

        Ok(())
    }

    /// Get the number of users currently on the site, including the ones that are about to leave the queue.
    fn get_on_site_count(&self) -> usize {
        self.local_on_site_list.len() + self.local_queue_leaving_list.len()
    }

    /// This function triggers an amount of QPID dequeue operations. The amount is the waiting room's minimum user count minus the current user count, provided in the parameter.
    /// If there are too many users on the site, this function will add dummy users to the queue, which will be dequeued by the QPID algorithm and thus lower the user count on the site.
    fn ensure_correct_site_count(&mut self, count: usize) -> Result<(), WaitingRoomError> {
        log::info!("[NODE {}] let users out of queue", self.node_id);
        if count < self.settings.min_user_count {
            log::debug!(
                "[NODE {}] not enough users on site, need to let {} users out of queue",
                self.node_id,
                self.settings.min_user_count - count
            );
            for _ in 0..(self.settings.min_user_count - count) {
                self.qpid_delete_min()?;
            }
        }

        if count > self.settings.max_user_count {
            log::debug!(
                "[NODE {}] too many users on site, need to add {} dummy users to the queue",
                self.node_id,
                count - self.settings.max_user_count
            );
            for _ in 0..(count - self.settings.max_user_count) {
                self.enqueue(Ticket::new_drain(self.node_id));
            }
        }

        Ok(())
    }
}
