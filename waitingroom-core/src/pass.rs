use serde::{Deserialize, Serialize};

use crate::{
    ticket::{Ticket, TicketIdentifier},
    NodeId, Time,
};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Pass {
    pub identifier: TicketIdentifier,
    pub node_id: NodeId,
    pub queue_join_time: Time,
    pub pass_creation_time: Time,
    pub expiry_time: Time,
}

impl Pass {
    pub fn from_ticket(ticket: Ticket, pass_expiry_time: Time) -> Self {
        let now_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        Self {
            identifier: ticket.identifier,
            node_id: ticket.node_id,
            queue_join_time: ticket.join_time,
            pass_creation_time: now_time,
            expiry_time: now_time + pass_expiry_time,
        }
    }

    pub fn refresh(&mut self, node_id: NodeId, pass_expiry_time: Time) {
        let now_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        self.node_id = node_id;
        self.expiry_time = now_time + pass_expiry_time;
    }
}
