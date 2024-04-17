use std::collections::BTreeMap;

use waitingroom_core::{
    ticket::{Ticket, TicketIdentifier},
    time::Time,
};

/// A queue of tickets. The ordering is based on the join time specified on the ticket.
/// The queue is implemented as a BTreeMap. It has a linear time complexity for finding
/// a ticket in the queue. This is not very efficient, but the local queue is not the
/// bottleneck in the system.
pub struct LocalQueue {
    queue: BTreeMap<(Time, TicketIdentifier), Ticket>,
}

impl LocalQueue {
    pub fn new() -> Self {
        Self {
            queue: BTreeMap::new(),
        }
    }

    /// Add a ticket to the queue.
    pub fn enqueue(&mut self, ticket: Ticket) {
        self.queue
            .insert((ticket.join_time, ticket.identifier), ticket);
    }

    /// Remove the ticket with the lowest join time from the queue.
    /// If the join time is equal, the ticket with the lowest identifier is removed.
    pub fn dequeue(&mut self) -> Option<Ticket> {
        self.queue.pop_first().map(|(_, ticket)| ticket)
    }

    /// Get a mutable reference to the ticket with the specified identifier.
    /// Used to update the ticket when it is refreshed.
    pub fn entry(&mut self, ticket_identifier: TicketIdentifier) -> Option<&mut Ticket> {
        self.queue.iter_mut().find_map(|(identifier, ticket)| {
            if identifier.1 == ticket_identifier {
                Some(ticket)
            } else {
                None
            }
        })
    }

    /// Returns the number of tickets in the queue.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Returns true if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Returns true if the queue contains a ticket with the specified identifier.
    pub fn contains(&self, ticket_identifier: TicketIdentifier) -> bool {
        self.queue
            .iter()
            .any(|(_, ticket)| ticket.identifier == ticket_identifier)
    }

    /// Returns the position of the ticket with the specified identifier.
    /// This is worst case O(n), where n is the number of tickets in the queue.
    /// [`LocalQueue::contains`] should be used if only the existence of the ticket is needed.
    pub fn get_position(&self, ticket_identifier: TicketIdentifier) -> Option<usize> {
        self.queue
            .iter()
            .position(|(_, ticket)| ticket.identifier == ticket_identifier)
    }

    /// Removes a ticket from the queue by its identifier.
    /// This is a linear search. If the ticket is not in the queue, None is returned.
    pub fn remove(&mut self, ticket_identifier: TicketIdentifier) -> Option<Ticket> {
        self.queue
            .iter()
            .find(|(_, ticket)| ticket.identifier == ticket_identifier)
            .map(|(identifier, _)| *identifier)
            .and_then(|identifier| self.queue.remove(&identifier))
    }

    /// Remove all elements where the ticket expiry time is less than the specified time.
    /// This is a linear time operation.
    pub fn remove_expired(&mut self, time: u128) -> u64 {
        let mut count = 0;
        self.queue.retain(|_, ticket| {
            if ticket.expiry_time < time {
                count += 1;
                false
            } else {
                true
            }
        });
        count
    }
}

impl Default for LocalQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_queue() {
        let mut queue = LocalQueue::new();

        let identifier0 = 77429;
        let identifier1 = 47156;
        let identifier2 = 81657;

        let ticket0 = Ticket::new_with_time_and_identifier(identifier0, 0, 0, 0, 0);
        let ticket1 = Ticket::new_with_time_and_identifier(identifier1, 1, 0, 0, 0);
        let ticket2 = Ticket::new_with_time_and_identifier(identifier2, 2, 0, 0, 0);

        queue.enqueue(ticket2);
        queue.enqueue(ticket0);
        queue.enqueue(ticket1);

        assert_eq!(queue.len(), 3);
        assert_eq!(queue.get_position(identifier0), Some(0));
        assert_eq!(queue.get_position(identifier1), Some(1));
        assert_eq!(queue.get_position(identifier2), Some(2));
        assert_eq!(queue.get_position(12345), None);
        assert_eq!(queue.dequeue(), Some(ticket0));
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.get_position(identifier0), None);
        assert_eq!(queue.get_position(identifier1), Some(0));
        assert_eq!(queue.get_position(identifier2), Some(1));
        assert_eq!(queue.dequeue(), Some(ticket1));
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.get_position(identifier0), None);
        assert_eq!(queue.get_position(identifier1), None);
        assert_eq!(queue.get_position(identifier2), Some(0));
        assert_eq!(queue.dequeue(), Some(ticket2));
        assert_eq!(queue.len(), 0);
        assert_eq!(queue.get_position(identifier0), None);
        assert_eq!(queue.get_position(identifier1), None);
        assert_eq!(queue.get_position(identifier2), None);
        assert_eq!(queue.dequeue(), None);
    }
}
