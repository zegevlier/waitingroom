use serde::{Deserialize, Serialize};

use crate::{
    random::RandomProvider,
    time::{Time, TimeProvider},
    NodeId,
};

pub type TicketIdentifier = u64;

/// Tickets are what users use to show that they are in the queue, and what position they
/// have in the queue. When used by the waiting room, they are fully trusted. Therefore,
/// if they are editable by the user, they should be signed to prevent tampering.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Ticket {
    /// The type of the ticket. This is either normal, skip or drain.
    /// Normal tickets are used for normal users, this should be the vast majority of tickets.
    ///
    /// Skip tickets are used for when a user at the front of the queue is kicked out. In this
    /// case, since it is unsafe in QPID to remove a this user from the queue, a ticket type
    /// is instead set to skip. This means that when the user is let out of the queue, their
    /// ticket will be skipped, and another user will be let out instead.
    ///
    /// Drain tickets are used when there are too many users on the site. If this happens, the
    /// operations that let users out of the queue will instead let out these phantom drain
    /// tickets, which, since they are not real users, will not actually let anyone out of the
    /// queue. This will cause the number of people on the site to decrease.
    pub ticket_type: TicketType,
    /// The ticket identifier is a random number used to uniquely identify a ticket.
    /// This same identifier is set on the pass the user gets when they are let out of the queue.
    pub identifier: TicketIdentifier,
    /// The time in milliseconds at which the user joined the queue.
    pub join_time: Time,
    /// The time in milliseconds at which the ticket should be refreshed next. They may refresh
    /// it sooner,but this is the time at which it should be refreshed automatically by the
    /// user's client.
    pub next_refresh_time: Time,
    /// The time in milliseconds at which the ticket will expire if it is not refreshed.
    /// The ticket will become invalid at this time.
    pub expiry_time: Time,
    /// The node ID where the ticket was last refreshed.
    pub node_id: NodeId,
    /// The previous position estimate of the user. If the current position estimate is
    /// greater than this, the user is still shown their previous position estimate to
    /// prevent them from seeing their position go up, as this would be very discouraging.
    /// Potentially, this could be made a configurable option in the future.
    pub previous_position_estimate: usize,
}

impl Ticket {
    pub fn new<T, R>(
        node_id: NodeId,
        ticket_refresh_time: Time,
        ticket_expiry_time: Time,
        time_provider: &T,
        random_provider: &R,
    ) -> Self
    where
        T: TimeProvider,
        R: RandomProvider,
    {
        let identifier = random_provider.random_u64();
        let now_time = time_provider.get_now_time();

        Self {
            identifier,
            join_time: now_time,
            next_refresh_time: now_time + ticket_refresh_time,
            expiry_time: now_time + ticket_expiry_time,
            node_id,
            previous_position_estimate: usize::MAX,
            ticket_type: TicketType::Normal,
        }
    }

    /// Creates a new ticket with the given identifier, join time, node ID, ticket refresh time
    /// and ticket expiry time. This is used only for testing purposes in `http-local-queue`.
    /// Generally, [`Ticket::new`] should be used instead.
    pub fn new_with_time_and_identifier(
        identifier: TicketIdentifier,
        join_time: Time,
        node_id: NodeId,
        ticket_refresh_time: Time,
        ticket_expiry_time: Time,
    ) -> Self {
        Self {
            identifier,
            join_time,
            next_refresh_time: join_time + ticket_refresh_time,
            expiry_time: join_time + ticket_expiry_time,
            node_id,
            previous_position_estimate: usize::MAX,
            ticket_type: TicketType::Normal,
        }
    }

    /// Sets the ticket type to skip. See [`Ticket::ticket_type`] for more information.
    pub fn set_skip(&mut self) {
        self.ticket_type = TicketType::Skip;
    }

    /// This creates a new drain ticket. See [`Ticket::ticket_type`] for more information.
    pub fn new_drain(node_id: NodeId) -> Self {
        let identifier = rand::random();

        Self {
            identifier,
            join_time: Time::MIN,
            next_refresh_time: Time::MIN,
            expiry_time: Time::MAX,
            node_id,
            previous_position_estimate: usize::MAX,
            ticket_type: TicketType::Drain,
        }
    }

    /// Refreshes the ticket. The ticket's position estimate is set to the given position
    /// estimate, and the ticket's refresh time and expiry time are updated.
    pub fn refresh<T>(
        &self,
        position_estimate: usize,
        ticket_refresh_time: Time,
        ticket_expiry_time: Time,
        time_provider: &T,
    ) -> Self
    where
        T: TimeProvider,
    {
        let now_time = time_provider.get_now_time();

        Self {
            identifier: self.identifier,
            join_time: self.join_time,
            next_refresh_time: now_time + ticket_refresh_time,
            expiry_time: now_time + ticket_expiry_time,
            node_id: self.node_id,
            previous_position_estimate: position_estimate,
            ticket_type: self.ticket_type,
        }
    }

    /// Returns true if the ticket is expired.
    pub fn is_expired<T>(&self, time_provider: &T) -> bool
    where
        T: TimeProvider,
    {
        let now_time = time_provider.get_now_time();

        self.expiry_time < now_time
    }
}

impl PartialEq for Ticket {
    fn eq(&self, other: &Self) -> bool {
        self.identifier == other.identifier
    }
}

impl Eq for Ticket {}

impl PartialOrd for Ticket {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ticket {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.join_time.cmp(&other.join_time)
    }
}

/// See the documentation for [`Ticket::ticket_type`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum TicketType {
    Normal,
    Drain,
    Skip,
}
