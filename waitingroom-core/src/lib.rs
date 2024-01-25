use pass::Pass;
use ticket::Ticket;

mod error;
pub mod pass;
pub mod ticket;

pub use error::WaitingRoomError;

pub(crate) const TICKET_REFRESH_TIME: Time = 5 * 1000;
pub(crate) const TICKET_EXPIRY_TIME: Time = TICKET_REFRESH_TIME * 2;
pub(crate) const PASS_EXPIRY_TIME: Time = 10 * 1000;

pub type Time = u128;
pub type NodeId = u64;

pub enum Identification {
    Ticket(Ticket),
    Pass(Pass),
}

pub trait WaitingRoomUserTriggered {
    fn join(&mut self) -> Result<Ticket, WaitingRoomError>;

    fn check_in(&mut self, ticket: Ticket) -> Result<CheckInResponse, WaitingRoomError>;

    fn leave(&mut self, ticket: Ticket) -> Result<Pass, WaitingRoomError>;

    fn disconnect(&mut self, identification: Identification);

    fn validate_and_refresh_pass(&mut self, pass: Pass) -> Result<Pass, WaitingRoomError>;
}

pub trait WaitingRoomTimerTriggered {
    fn cleanup(&mut self) -> Result<(), WaitingRoomError>;

    fn sync_user_counts(&mut self) -> Result<(), WaitingRoomError>;

    fn ensure_correct_user_count(&mut self) -> Result<(), WaitingRoomError>;
}

pub trait WaitingRoomMessageTriggered {
    // TODO: Add message types.
    // These are all not needed for the basic version, since we only have a single node.
}

pub struct CheckInResponse {
    pub new_ticket: Ticket,
    pub position_estimate: usize,
}
