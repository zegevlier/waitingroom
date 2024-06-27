use waitingroom_core::{
    pass::Pass,
    random::RandomProvider,
    settings,
    ticket::{Ticket, TicketIdentifier, TicketType},
    time::TimeProvider,
    NodeId, WaitingRoomError, WaitingRoomMessageTriggered, WaitingRoomTimerTriggered,
    WaitingRoomUserTriggered,
};
use waitingroom_local_queue::LocalQueue;

pub use settings::GeneralWaitingRoomSettings;

/// Since we always only have a single node in the basic waiting rooms,
/// we just hardcode the node id as 0.
const SELF_NODE_ID: NodeId = 0;

/// This is a very basic implementation of a waiting room.
/// It only supports a single node. It's useful for testing.
pub struct BasicWaitingRoom<T, R>
where
    T: TimeProvider,
    R: RandomProvider,
{
    local_queue: LocalQueue,
    queue_leaving_list: Vec<Ticket>,
    on_site_list: Vec<Pass>,

    settings: GeneralWaitingRoomSettings,

    time_provider: T,
    random_provider: R,
}

impl<T, R> WaitingRoomUserTriggered for BasicWaitingRoom<T, R>
where
    T: TimeProvider,
    R: RandomProvider,
{
    fn join(&mut self) -> Result<waitingroom_core::ticket::Ticket, WaitingRoomError> {
        let ticket = waitingroom_core::ticket::Ticket::new(
            SELF_NODE_ID,
            self.settings.ticket_refresh_time,
            self.settings.ticket_expiry_time,
            &self.time_provider,
            &self.random_provider,
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

        if ticket.node_id != SELF_NODE_ID {
            // This should never happen, since we only have a single node.
            // But, if it does, we need to add the ticket to the local queue.
            self.enqueue(ticket);
        }

        let position_estimate = match self.local_queue.get_position(ticket.identifier) {
            Some(position) => position + 1, // 0 is reserved for users who are allowed to leave the queue.
            None => {
                if self.queue_leaving_list.contains(&ticket) {
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
                    .queue_leaving_list
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
                    SELF_NODE_ID,
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

        if ticket.node_id != SELF_NODE_ID {
            // This should never happen, since we only have a single node.
            // But, if it does, the user will need to re-join the queue.
            return Err(WaitingRoomError::TicketAtWrongNode);
        }

        if !self.queue_leaving_list.contains(&ticket) {
            // The user is not allowed to leave the queue yet.
            return Err(WaitingRoomError::TicketCannotLeaveYet);
        }

        // The user is allowed to leave the queue.
        // We remove the ticket from the queue leaving list.
        self.queue_leaving_list.retain(|t| t != &ticket);
        // We know the number of items removed here is always 1.
        metrics::gauge!(
            "waitingroom.to_let_in_count",
            &[("node", SELF_NODE_ID.to_string())]
        )
        .decrement(1);

        // Generate a pass for the user.
        let pass = Pass::from_ticket(ticket, self.settings.pass_expiry_time, &self.time_provider);

        // And add the pass to the users on site list.
        self.on_site_list.push(pass);
        metrics::gauge!(
            "waitingroom.on_site_count",
            &[("node", SELF_NODE_ID.to_string())]
        )
        .increment(1);

        Ok(pass)
    }

    fn validate_and_refresh_pass(
        &mut self,
        pass: waitingroom_core::pass::Pass,
    ) -> Result<waitingroom_core::pass::Pass, WaitingRoomError> {
        let now_time = self.time_provider.get_now_time();

        if pass.expiry_time < now_time {
            return Err(WaitingRoomError::PassExpired);
        }

        if pass.node_id != SELF_NODE_ID {
            self.on_site_list.push(pass);
            metrics::gauge!(
                "waitingroom.on_site_count",
                &[("node", SELF_NODE_ID.to_string())]
            )
            .increment(1);
        }

        let pass = self
            .on_site_list
            .iter_mut()
            .find(|p| p.identifier == pass.identifier)
            .map(|pass| {
                *pass = pass.refresh(
                    SELF_NODE_ID,
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

impl<T, R> WaitingRoomTimerTriggered for BasicWaitingRoom<T, R>
where
    T: TimeProvider,
    R: RandomProvider,
{
    fn cleanup(&mut self) -> Result<(), WaitingRoomError> {
        let now_time = self.time_provider.get_now_time();

        // Remove expired tickets from the local queue.
        let removed_count = self.local_queue.remove_expired(now_time);
        metrics::gauge!(
            "waitingroom.in_queue_count",
            &[("node", SELF_NODE_ID.to_string())]
        )
        .decrement(removed_count as f64);

        self.on_site_list.retain(|pass| pass.expiry_time > now_time);
        metrics::gauge!(
            "waitingroom.on_site_count",
            &[("node", SELF_NODE_ID.to_string())]
        )
        .set(self.on_site_list.len() as f64);

        // TODO: Replace this with something in an operation queue.
        // This method should not be called inside another method.
        self.let_users_out_of_queue(removed_count as usize)?;

        self.queue_leaving_list
            .retain(|ticket| ticket.expiry_time > now_time);
        metrics::gauge!(
            "waitingroom.to_let_in_count",
            &[("node", SELF_NODE_ID.to_string())]
        )
        .set(self.queue_leaving_list.len() as f64);

        Ok(())
    }

    fn eviction(&mut self) -> Result<(), WaitingRoomError> {
        // We use this user count, because people that are about to leave the queue
        // should be counted as users on site.
        let user_count = self.on_site_list.len() + self.queue_leaving_list.len();

        // If there are too few users on site, let users out of the queue.
        if user_count < self.settings.min_user_count {
            self.let_users_out_of_queue(self.settings.min_user_count - self.on_site_list.len())?;
        }
        // If there are too many users on the site, add dummy users to the queue.
        if user_count > self.settings.max_user_count {
            // This should never happen
            for _ in 0..(self.on_site_list.len() - self.settings.max_user_count) {
                self.enqueue(Ticket::new_drain(SELF_NODE_ID));
            }
        }

        Ok(())
    }

    fn fault_detection(&mut self) -> Result<(), WaitingRoomError> {
        unimplemented!("Fault detection is not implemented for the basic waiting room.")
    }
}

// Since the basic waiting room only has a single node, these are all unreachable, since they should never be called.
impl<T, R> WaitingRoomMessageTriggered for BasicWaitingRoom<T, R>
where
    T: TimeProvider,
    R: RandomProvider,
{
}

impl<T, R> BasicWaitingRoom<T, R>
where
    T: TimeProvider,
    R: RandomProvider,
{
    pub fn new(settings: GeneralWaitingRoomSettings, time_provider: T, random_provider: R) -> Self {
        Self {
            local_queue: LocalQueue::new(),
            queue_leaving_list: Vec::new(),
            on_site_list: Vec::new(),
            time_provider,
            random_provider,
            settings,
        }
    }

    pub fn let_users_out_of_queue(&mut self, count: usize) -> Result<(), WaitingRoomError> {
        // Get the first `count` tickets from the local queue.
        let mut tickets = (0..count)
            .filter_map(|_| self.dequeue())
            .collect::<Vec<_>>();

        let mut idx = 0;
        while idx < tickets.len() {
            let ticket = tickets[idx];
            match ticket.ticket_type {
                TicketType::Normal => {
                    self.queue_leaving_list.push(ticket);
                    metrics::gauge!(
                        "waitingroom.to_let_in_count",
                        &[("node", SELF_NODE_ID.to_string())]
                    )
                    .increment(1);
                }
                TicketType::Drain => {
                    // This ticket is a dummy ticket. We shouldn't do anything with it.
                }
                TicketType::Skip => {
                    // For this ticket, we need to take someone else out of the queue.
                    if let Some(ticket) = self.dequeue() {
                        tickets.push(ticket);
                    }
                }
            }

            idx += 1;
        }

        Ok(())
    }

    pub fn get_user_count(&self) -> usize {
        self.local_queue.len()
    }

    /// Add a ticket to the local queue, incrementing the metric if the ticket type is normal.
    pub fn enqueue(&mut self, ticket: Ticket) {
        self.local_queue.enqueue(ticket);
        if ticket.ticket_type == TicketType::Normal {
            metrics::gauge!(
                "waitingroom.in_queue_count",
                &[("node", SELF_NODE_ID.to_string())]
            )
            .increment(1);
        }
    }

    /// Remove the element at the front of the local queue, decrementing the metric if the ticket type is normal.
    pub fn dequeue(&mut self) -> Option<Ticket> {
        let element = self.local_queue.dequeue();
        if element.is_some() && element.as_ref().unwrap().ticket_type == TicketType::Normal {
            metrics::gauge!(
                "waitingroom.in_queue_count",
                &[("node", SELF_NODE_ID.to_string())]
            )
            .decrement(1);
        }
        element
    }

    // / Remove a specific element from the local queue by identifier, decrementing the metric if the ticket type is normal.
    pub fn remove_from_queue(&mut self, ticket_identifier: TicketIdentifier) {
        if let Some(ticket) = self.local_queue.remove(ticket_identifier) {
            if ticket.ticket_type == TicketType::Normal {
                metrics::gauge!(
                    "waitingroom.in_queue_count",
                    &[("node", SELF_NODE_ID.to_string())]
                )
                .decrement(1);
            }
        }
    }
}
