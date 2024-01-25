use serde::{Deserialize, Serialize};

use crate::{
    ticket::{Ticket, TicketIdentifier},
    Time,
};

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Pass {
    pub identifier: TicketIdentifier,
    pub node_id: u64,
    pub queue_join_time: Time,
    pub pass_creation_time: Time,
    pub expiry_time: Time,
}

impl Pass {
    pub fn from_ticket(ticket: Ticket) -> Self {
        let now_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        Self {
            identifier: ticket.identifier,
            node_id: ticket.node_id,
            queue_join_time: ticket.join_time,
            pass_creation_time: now_time,
            expiry_time: now_time + crate::PASS_EXPIRY_TIME,
        }
    }

    pub fn refresh(&mut self, node_id: u64) {
        let now_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        self.node_id = node_id;
        self.expiry_time = now_time + crate::PASS_EXPIRY_TIME;
    }
}
