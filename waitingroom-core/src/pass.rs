use serde::{Deserialize, Serialize};

use crate::{
    ticket::{Ticket, TicketIdentifier},
    time::TimeProvider,
    NodeId, Time,
};

/// The user gets a pass when they leave the queue.
/// It is used to show that they are allowed to visit the site.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Pass {
    /// The identifier of the ticket that this pass was created from.
    pub identifier: TicketIdentifier,
    /// The node id the pass was last refreshed on.
    pub node_id: NodeId,
    /// The time the original ticket was added to the queue.
    pub queue_join_time: Time,
    /// The time the pass was created.
    pub pass_creation_time: Time,
    /// The time the pass expires if it is not refreshed.
    pub expiry_time: Time,
}

impl Pass {
    /// Creates a new pass from a ticket. The pass will expire after `pass_expiry_time` milliseconds.
    /// The pass is created at the current time, and the expiry time is calculated from that.
    /// The node id, identifier and queue join time are gotten from the ticket.
    pub fn from_ticket<T>(ticket: Ticket, pass_expiry_time: Time, time_provider: &T) -> Self
    where
        T: TimeProvider,
    {
        let now_time = time_provider.get_now_time();

        Self {
            identifier: ticket.identifier,
            node_id: ticket.node_id,
            queue_join_time: ticket.join_time,
            pass_creation_time: now_time,
            expiry_time: now_time + pass_expiry_time,
        }
    }

    /// Refreshes the pass, setting the expiry time to the current time, plus
    /// the `pass_expiry_time` and setting the node id to `node_id`.
    pub fn refresh<T>(&self, node_id: NodeId, pass_expiry_time: Time, time_provider: &T) -> Self
    where
        T: TimeProvider,
    {
        let now_time = time_provider.get_now_time();

        Self {
            node_id,
            expiry_time: now_time + pass_expiry_time,
            identifier: self.identifier,
            queue_join_time: self.queue_join_time,
            pass_creation_time: self.pass_creation_time,
        }
    }
}
