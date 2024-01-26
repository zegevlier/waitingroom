use waitingroom_core::{
    pass::Pass,
    ticket::{self, Ticket, TicketIdentifier},
    NodeId, WaitingRoomError, WaitingRoomMessageTriggered, WaitingRoomTimerTriggered,
    WaitingRoomUserTriggered,
};
use waitingroom_local_queue::LocalQueue;

mod metrics;
mod settings;

pub use settings::BasicWaitingRoomSettings;

/// Since we always only have a single node in the basic waiting rooms,
/// we just hardcode the node id as 0.
const SELF_NODE_ID: NodeId = 0;

/// This is a very basic implementation of a waiting room.
/// It only supports a single node. It's useful for testing.
pub struct BasicWaitingRoom {
    local_queue: LocalQueue,
    queue_leaving_list: Vec<Ticket>,
    on_site_list: Vec<Pass>,

    settings: BasicWaitingRoomSettings,
}

impl WaitingRoomUserTriggered for BasicWaitingRoom {
    fn join(&mut self) -> Result<waitingroom_core::ticket::Ticket, WaitingRoomError> {
        let ticket = waitingroom_core::ticket::Ticket::new(
            SELF_NODE_ID,
            self.settings.ticket_refresh_time,
            self.settings.ticket_expiry_time,
        );
        self.enqueue(ticket);
        Ok(ticket)
    }

    fn check_in(
        &mut self,
        ticket: waitingroom_core::ticket::Ticket,
    ) -> Result<waitingroom_core::CheckInResponse, WaitingRoomError> {
        if ticket.is_expired() {
            // This happens when a user has not refreshed their ticket in time.
            return Err(WaitingRoomError::TicketExpired);
        }

        if ticket.node_id != SELF_NODE_ID {
            // This should never happen, since we only have a single node.
            // But, if it does, we need to add the ticket to the local queue.
            self.enqueue(ticket);
        }

        let position_estimate = match self.local_queue.get_position(ticket.identifier) {
            Some(position) => position.max(1), // 0 is reserved for users who are allowed to leave the queue.
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

        // call frefresh on the ticket
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
        if ticket.is_expired() {
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

        // Generate a pass for the user.
        let pass = Pass::from_ticket(ticket, self.settings.pass_expiry_time);

        // And add the pass to the users on site list.
        self.on_site_list.push(pass);

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
                    self.queue_leaving_list.retain(|t| t != &ticket);
                }
            }
            waitingroom_core::Identification::Pass(pass) => {
                self.on_site_list
                    .retain(|p| p.identifier != pass.identifier);
            }
        }

        Ok(())
    }

    fn validate_and_refresh_pass(
        &mut self,
        pass: waitingroom_core::pass::Pass,
    ) -> Result<waitingroom_core::pass::Pass, WaitingRoomError> {
        let now_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        if pass.expiry_time < now_time {
            return Err(WaitingRoomError::PassExpired);
        }

        if pass.node_id != SELF_NODE_ID {
            self.on_site_list.push(pass);
        }

        if let Some(pass) = self
            .on_site_list
            .iter_mut()
            .find(|p| p.identifier == pass.identifier)
        {
            pass.refresh(SELF_NODE_ID, self.settings.pass_expiry_time);
        };

        Ok(pass)
    }
}

impl WaitingRoomTimerTriggered for BasicWaitingRoom {
    fn cleanup(&mut self) -> Result<(), WaitingRoomError> {
        let now_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        // Remove expired tickets from the local queue.
        let amount_removed = self.local_queue.remove_expired(now_time);
        metrics::waitingroom_basic::in_queue_count(SELF_NODE_ID).dec_by(amount_removed);

        // Remove expired passes from the on site list.
        let mut removed_count = 0;
        self.on_site_list.retain(|pass| {
            if pass.expiry_time > now_time {
                true
            } else {
                removed_count += 1;
                false
            }
        });

        self.let_users_out_of_queue(removed_count)?;

        // Remove expired tickets from the queue leaving list.
        self.queue_leaving_list
            .retain(|ticket| ticket.expiry_time > now_time);

        Ok(())
    }

    fn sync_user_counts(&mut self) -> Result<(), WaitingRoomError> {
        // This is a no-op, since there is only a single node.
        // Nothing needs to be synced.
        Ok(())
    }

    fn ensure_correct_user_count(&mut self) -> Result<(), WaitingRoomError> {
        // If there are too few users on site, let users out of the queue.
        if self.on_site_list.len() < self.settings.min_user_count {
            self.let_users_out_of_queue(self.settings.min_user_count - self.on_site_list.len())?;
        }
        // If there are too many users on the site, add dummy users to the queue.
        if self.on_site_list.len() > self.settings.max_user_count {
            // This should never happen
            for _ in 0..(self.on_site_list.len() - self.settings.max_user_count) {
                self.enqueue(Ticket::new_drain(SELF_NODE_ID));
            }
        }

        Ok(())
    }
}

// Since the basic waiting room only has a single node, these are all unreachables, since they should never be called.
impl WaitingRoomMessageTriggered for BasicWaitingRoom {}

impl BasicWaitingRoom {
    pub fn new(settings: BasicWaitingRoomSettings) -> Self {
        Self {
            local_queue: LocalQueue::new(),
            queue_leaving_list: Vec::new(),
            on_site_list: Vec::new(),
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
                ticket::TicketType::Normal => {
                    self.queue_leaving_list.push(ticket);
                }
                ticket::TicketType::Drain => {
                    // This ticket is a dummy ticket. We shouldn't do anything with it.
                }
                ticket::TicketType::Skip => {
                    // For this ticket, we need to take someone else out of the queue.
                    if let Some(ticket) = self.dequeue() {
                        tickets.push(ticket);
                    }
                }
            }

            idx += 1;
        }

        // Add the tickets to the queue leaving list.
        self.queue_leaving_list.extend(tickets);

        Ok(())
    }

    pub fn get_user_count(&self) -> usize {
        self.local_queue.len()
    }

    pub fn enqueue(&mut self, ticket: Ticket) {
        self.local_queue.enqueue(ticket);
        if ticket.ticket_type == ticket::TicketType::Normal {
            metrics::waitingroom_basic::in_queue_count(SELF_NODE_ID).inc();
        }
    }

    pub fn dequeue(&mut self) -> Option<Ticket> {
        let element = self.local_queue.dequeue();
        if element.is_some() && element.as_ref().unwrap().ticket_type == ticket::TicketType::Normal
        {
            metrics::waitingroom_basic::in_queue_count(SELF_NODE_ID).dec();
        }
        element
    }

    pub fn remove_from_queue(&mut self, ticket_identifier: TicketIdentifier) {
        if self.local_queue.remove(ticket_identifier).is_some() {
            metrics::waitingroom_basic::in_queue_count(SELF_NODE_ID).dec();
        }
    }

    pub fn get_cleanup_interval(&self) -> u128 {
        self.settings.cleanup_interval
    }

    pub fn get_sync_interval(&self) -> u128 {
        self.settings.sync_user_counts_interval
    }

    pub fn get_ensure_correct_count_interval(&self) -> u128 {
        self.settings.ensure_correct_user_count_interval
    }
}
