use serde::{Deserialize, Serialize};

use crate::{NodeId, Time};

pub type TicketIdentifier = u64;

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Ticket {
    pub ticket_type: TicketType,
    pub identifier: TicketIdentifier,
    pub join_time: Time,
    pub next_refresh_time: Time,
    pub expiry_time: Time,
    pub node_id: NodeId,
    pub previous_position_estimate: usize,
}

impl Ticket {
    pub fn new(node_id: NodeId, ticket_refresh_time: Time, ticket_expiry_time: Time) -> Self {
        let identifier = rand::random();
        let now_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

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

    pub fn new_with_time_and_identifier(
        identifier: TicketIdentifier,
        join_time: Time,
        node_id: NodeId,
        ticket_refresh_time: Time,
        ticket_expiry_time: Time,
    ) -> Self {
        let now_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        Self {
            identifier,
            join_time,
            next_refresh_time: now_time + ticket_refresh_time,
            expiry_time: now_time + ticket_expiry_time,
            node_id,
            previous_position_estimate: usize::MAX,
            ticket_type: TicketType::Normal,
        }
    }

    pub fn set_skip(&mut self) {
        self.ticket_type = TicketType::Skip;
    }

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

    pub fn refresh(
        &self,
        position_estimate: usize,
        ticket_refresh_time: Time,
        ticket_expiry_time: Time,
    ) -> Self {
        let now_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

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

    pub fn is_expired(&self) -> bool {
        let now_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum TicketType {
    Normal,
    Drain,
    Skip,
}
