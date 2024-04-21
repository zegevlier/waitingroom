use messages::NodeToNodeMessage;
use waitingroom_core::{
    metrics,
    network::{Network, NetworkHandle},
    pass::Pass,
    retain_with_count, settings,
    ticket::{Ticket, TicketIdentifier, TicketType},
    time::{Time, TimeProvider},
    NodeId, WaitingRoomError, WaitingRoomMessageTriggered, WaitingRoomTimerTriggered,
    WaitingRoomUserTriggered,
};
use waitingroom_local_queue::LocalQueue;

pub use settings::GeneralWaitingRoomSettings;

#[cfg(test)]
mod test;

pub mod messages;

/// This is the waiting room implementation described in the associated paper.
pub struct DistributedWaitingRoom<T, N>
where
    T: TimeProvider,
    N: Network<NodeToNodeMessage>,
{
    local_queue: LocalQueue,
    local_queue_leaving_list: Vec<Ticket>,
    local_on_site_list: Vec<Pass>,

    settings: GeneralWaitingRoomSettings,
    node_id: NodeId,

    network_handle: N::NetworkHandle,

    time_provider: T,

    qpid_parent: Option<NodeId>,
    qpid_current_weight: Time,
    qpid_weight_table: Vec<(NodeId, Time)>,
}

impl<T, N> WaitingRoomUserTriggered for DistributedWaitingRoom<T, N>
where
    T: TimeProvider,
    N: Network<NodeToNodeMessage>,
{
    fn join(&mut self) -> Result<waitingroom_core::ticket::Ticket, WaitingRoomError> {
        let ticket = waitingroom_core::ticket::Ticket::new(
            self.node_id,
            self.settings.ticket_refresh_time,
            self.settings.ticket_expiry_time,
            &self.time_provider,
        );
        self.enqueue(ticket);
        Ok(ticket)
    }

    fn check_in(
        &mut self,
        ticket: waitingroom_core::ticket::Ticket,
    ) -> Result<waitingroom_core::CheckInResponse, WaitingRoomError> {
        if ticket.is_expired(&self.time_provider) {
            // This happens when a user has not refreshed their ticket in time.
            return Err(WaitingRoomError::TicketExpired);
        }

        if ticket.node_id != self.node_id {
            // This should never happen, since we only have a single node.
            // But, if it does, we need to add the ticket to the local queue.
            self.enqueue(ticket);
        }

        let position_estimate = match self.local_queue.get_position(ticket.identifier) {
            Some(position) => position + 1, // 0 is reserved for users who are allowed to leave the queue.
            None => {
                if self.local_queue_leaving_list.contains(&ticket) {
                    // The ticket is in the queue leaving list.
                    // This means that the user can now leave the queue.
                    // When this happens, we send the user's position estimate as 0.
                    0
                } else {
                    // The ticket is not in the queue leaving list.
                    // This usually means the ticket has already been used to leave the queue.
                    // They can't use this ticket again, so it is invalid.
                    return Err(WaitingRoomError::TicketNotInQueue);
                }
            }
        };

        let position_estimate = if position_estimate > ticket.previous_position_estimate {
            // The ticket has moved backwards in the queue.
            // This should never happen with a single node, but may happen with multiple nodes.
            // If it does, we need to send the user's old position estimate.
            ticket.previous_position_estimate
        } else {
            position_estimate
        };

        // call refresh on the ticket
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
        if ticket.is_expired(&self.time_provider) {
            // This happens when a user has not refreshed their ticket in time.
            return Err(WaitingRoomError::TicketExpired);
        }

        if ticket.node_id != self.node_id {
            // This should never happen, since we only have a single node.
            // But, if it does, the user will need to re-join the queue.
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

    fn disconnect(
        &mut self,
        identification: waitingroom_core::Identification,
    ) -> Result<(), WaitingRoomError> {
        match identification {
            waitingroom_core::Identification::Ticket(ticket) => {
                if self.local_queue.contains(ticket.identifier) {
                    self.remove_from_queue(ticket.identifier);
                } else {
                    // Since we don't know whether the value is in the queue, and we cannot assume it is actually removed,
                    // we count the number of items removed from the list (either 0 or 1) and decrement the metric by that.
                    let removed_count =
                        retain_with_count(&mut self.local_queue_leaving_list, |t| t != &ticket);
                    metrics::waitingroom::to_be_let_in_count(self.node_id).dec_by(removed_count);
                }
            }
            waitingroom_core::Identification::Pass(pass) => {
                let removed_count = retain_with_count(&mut self.local_on_site_list, |p| {
                    p.identifier != pass.identifier
                });
                metrics::waitingroom::on_site_count(self.node_id).dec_by(removed_count);
            }
        }

        Ok(())
    }

    fn validate_and_refresh_pass(
        &mut self,
        pass: waitingroom_core::pass::Pass,
    ) -> Result<waitingroom_core::pass::Pass, WaitingRoomError> {
        let now_time = self.time_provider.get_now_time();

        if pass.expiry_time < now_time {
            return Err(WaitingRoomError::PassExpired);
        }

        if pass.node_id != self.node_id {
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
            None => Err(WaitingRoomError::PassNotInList),
        }
    }
}

impl<T, N> WaitingRoomTimerTriggered for DistributedWaitingRoom<T, N>
where
    T: TimeProvider,
    N: Network<NodeToNodeMessage>,
{
    fn cleanup(&mut self) -> Result<(), WaitingRoomError> {
        let now_time = self.time_provider.get_now_time();

        // Remove expired tickets from the local queue.
        let removed_count = self.local_queue.remove_expired(now_time);
        metrics::waitingroom::in_queue_count(self.node_id).dec_by(removed_count);

        // Remove expired passes from the on site list.
        let removed_count = retain_with_count(&mut self.local_on_site_list, |pass| {
            pass.expiry_time > now_time
        });
        metrics::waitingroom::on_site_count(self.node_id).dec_by(removed_count);

        // TODO: Replace this with the correct method when QPID is implemented.
        // This method should not be called inside another method.
        // self.let_users_out_of_queue(removed_count as usize)?;

        // Remove expired tickets from the queue leaving list.
        let removed_count = retain_with_count(&mut self.local_queue_leaving_list, |ticket| {
            ticket.expiry_time > now_time
        });
        metrics::waitingroom::to_be_let_in_count(self.node_id).dec_by(removed_count);

        Ok(())
    }

    fn sync_user_counts(&mut self) -> Result<(), WaitingRoomError> {
        // This is a no-op, since there is only a single node.
        // Nothing needs to be synced.
        Ok(())
    }

    fn ensure_correct_user_count(&mut self) -> Result<(), WaitingRoomError> {
        // We use this user count, because people that are about to leave the queue
        // should be counted as users on site.
        let user_count = self.local_on_site_list.len() + self.local_queue_leaving_list.len();

        // If there are too few users on site, let users out of the queue.
        if user_count < self.settings.min_user_count {
            for _ in 0..(self.settings.min_user_count - self.local_on_site_list.len()) {
                self.qpid_delete_min()?;
            }
        }
        // If there are too many users on the site, add dummy users to the queue.
        if user_count > self.settings.max_user_count {
            // This should never happen
            for _ in 0..(self.local_on_site_list.len() - self.settings.max_user_count) {
                self.enqueue(Ticket::new_drain(self.node_id));
            }
        }

        Ok(())
    }
}

// Since the basic waiting room only has a single node, these are all unreachable, since they should never be called.
impl<T, N> WaitingRoomMessageTriggered for DistributedWaitingRoom<T, N>
where
    T: TimeProvider,
    N: Network<NodeToNodeMessage>,
{
    fn receive_message(&mut self) -> Result<bool, WaitingRoomError> {
        if let Some(message) = self.network_handle.receive_message().unwrap() {
            match message.message {
                NodeToNodeMessage::QPIDUpdateMessage(weight) => {
                    self.qpid_handle_update(message.from_node, weight)
                }
                NodeToNodeMessage::QPIDDeleteMin => self.qpid_delete_min(),
                NodeToNodeMessage::QPIDFindRootMessage(weight) => {
                    self.qpid_handle_find_root(message.from_node, weight)
                }
            }?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl<T, N> DistributedWaitingRoom<T, N>
where
    T: TimeProvider,
    N: Network<NodeToNodeMessage>,
{
    pub fn new(
        settings: GeneralWaitingRoomSettings,
        node_id: NodeId,
        time_provider: T,
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
            local_queue_leaving_list: Vec::new(),
            local_on_site_list: Vec::new(),
            qpid_current_weight: Time::MAX,
            qpid_parent: Some(2), // TODO: This should be None before QPID is initialized.
            qpid_weight_table: vec![(1, Time::MAX), (2, Time::MAX)], // TODO: This should be empty before QPID is initialized.
            node_id,
            time_provider,
            settings,
            network_handle,
        }
    }

    pub fn get_user_count(&self) -> usize {
        self.local_queue.len()
    }

    /// Add a ticket to the local queue, incrementing the metric if the ticket type is normal.
    pub fn enqueue(&mut self, ticket: Ticket) {
        self.local_queue.enqueue(ticket);
        if ticket.ticket_type == TicketType::Normal {
            metrics::waitingroom::in_queue_count(self.node_id).inc();
        }
        if ticket.join_time < self.qpid_current_weight {
            self.qpid_insert(ticket.join_time);
        }
    }

    /// Remove the element at the front of the local queue, decrementing the metric if the ticket type is normal.
    pub fn dequeue(&mut self) -> Option<Ticket> {
        let element = self.local_queue.dequeue();
        if element.is_some() && element.as_ref().unwrap().ticket_type == TicketType::Normal {
            metrics::waitingroom::in_queue_count(self.node_id).dec();
        }
        element
    }

    fn qpid_insert(&mut self, weight: Time) {
        // TODO QPID counter

        self.qpid_current_weight = weight;
        if let Some(qpid_parent) = self.qpid_parent {
            if qpid_parent == self.node_id {
                return;
            }
            let updated_weight = self.qpid_compute_weight(qpid_parent);
            if updated_weight != self.qpid_get_from_weight_table(qpid_parent).unwrap() {
                self.qpid_set_in_weight_table(qpid_parent, updated_weight);
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

    fn qpid_handle_update(
        &mut self,
        from_node: NodeId,
        weight: Time,
    ) -> Result<(), WaitingRoomError> {
        // TODO: QPID counter

        self.qpid_set_in_weight_table(from_node, weight);
        if self.qpid_parent == Some(self.node_id) {
            if weight < self.qpid_current_weight {
                self.qpid_parent = Some(from_node);
                let updated_weight = self.qpid_compute_weight(from_node);
                self.qpid_set_in_weight_table(from_node, updated_weight);
                self.network_handle
                    .send_message(
                        from_node,
                        NodeToNodeMessage::QPIDFindRootMessage(updated_weight),
                    )
                    .unwrap()
            }
        } else {
            let updated_weight = self.qpid_compute_weight(self.qpid_parent.unwrap());
            if updated_weight
                != self
                    .qpid_get_from_weight_table(self.qpid_parent.unwrap())
                    .unwrap()
            {
                self.qpid_set_in_weight_table(self.qpid_parent.unwrap(), updated_weight);
                self.network_handle
                    .send_message(
                        from_node,
                        NodeToNodeMessage::QPIDUpdateMessage(updated_weight),
                    )
                    .unwrap()
            }
        }

        Ok(())
    }

    fn qpid_handle_find_root(
        &mut self,
        from_node: NodeId,
        weight: Time,
    ) -> Result<(), WaitingRoomError> {
        // TODO QPID Counter

        self.qpid_set_in_weight_table(from_node, weight);
        self.qpid_parent = Some(self.qpid_get_smallest_weight_node());
        if self.qpid_parent.unwrap() != self.node_id {
            let updated_weight = self.qpid_compute_weight(self.qpid_parent.unwrap());
            self.qpid_set_in_weight_table(self.qpid_parent.unwrap(), updated_weight);
            self.network_handle
                .send_message(
                    from_node,
                    NodeToNodeMessage::QPIDUpdateMessage(updated_weight),
                )
                .unwrap()
        }
        Ok(())
    }

    fn qpid_get_from_weight_table(&self, node_id: NodeId) -> Option<Time> {
        self.qpid_weight_table
            .iter()
            .find(|(id, _)| *id == node_id)
            .map(|(_, weight)| *weight)
    }

    fn qpid_set_in_weight_table(&mut self, node_id: NodeId, value: Time) {
        if let Some(v) = self
            .qpid_weight_table
            .iter_mut()
            .find(|(id, _)| *id == node_id)
        {
            v.1 = value;
        }
    }

    fn qpid_compute_weight(&self, node_id: NodeId) -> Time {
        self.qpid_weight_table
            .iter()
            .filter(|(id, _)| *id != node_id)
            .fold(self.qpid_current_weight, |min_weight, (_, weight)| {
                min_weight.min(*weight)
            })
    }

    fn qpid_get_smallest_weight_node(&self) -> NodeId {
        self.qpid_weight_table
            .iter()
            .min_by_key(|(_, weight)| *weight)
            .map(|(id, _)| *id)
            .unwrap()
    }

    pub fn qpid_delete_min(&mut self) -> Result<(), WaitingRoomError> {
        if self.qpid_parent.is_none() {
            return Err(WaitingRoomError::QPIDNotInitialized);
        }
        let qpid_parent = self.qpid_parent.unwrap();
        if qpid_parent != self.node_id {
            self.network_handle
                .send_message(qpid_parent, NodeToNodeMessage::QPIDDeleteMin)
                .unwrap();
        }

        if self.local_queue.is_empty() {
            return Ok(());
        }

        let ticket = self.dequeue().unwrap();

        // Update current QPID weight
        match self.local_queue.peek() {
            Some(next_ticket) => {
                self.qpid_current_weight = next_ticket.join_time;
            }
            None => {
                self.qpid_current_weight = Time::MAX;
            }
        }

        if self
            .qpid_weight_table
            .iter()
            .any(|(_, weight)| *weight != Time::MAX)
        {
            self.qpid_parent = Some(self.qpid_get_smallest_weight_node());
            if self.qpid_parent.unwrap() != self.node_id {
                let updated_weight = self.qpid_compute_weight(self.qpid_parent.unwrap());
                self.qpid_set_in_weight_table(self.qpid_parent.unwrap(), updated_weight);
                self.network_handle
                    .send_message(
                        self.qpid_parent.unwrap(),
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

    /// Remove a specific element from the local queue by identifier, decrementing the metric if the ticket type is normal.
    pub fn remove_from_queue(&mut self, ticket_identifier: TicketIdentifier) {
        if let Some(ticket) = self.local_queue.remove(ticket_identifier) {
            if ticket.ticket_type == TicketType::Normal {
                metrics::waitingroom::in_queue_count(self.node_id).dec();
            }
        }
    }
}
