use pass::Pass;
use ticket::Ticket;

mod error;
pub mod network;
pub mod pass;
pub mod random;
pub mod settings;
pub mod ticket;
pub mod time;

pub use error::WaitingRoomError;

/// The type for node identifiers. This is specified here to allow for easy changes in the future.
pub type NodeId = usize;

/// These functions are able to be triggered by actions from the user.
/// In most implementations, they will be called by a server on behalf of the user.
pub trait WaitingRoomUserTriggered {
    /// This is the first function the user should call when they want to join the waiting room.
    /// It returns a ticket that the user can use to check in and eventually leave the waiting room.
    fn join(&mut self) -> Result<Ticket, WaitingRoomError>;

    /// This is the function the user should call periodically to refresh their ticket and
    /// get an updated position estimate. If the estimated position is 0, the user should
    /// call [`WaitingRoomUserTriggered::leave`], since they are at the front of the queue.
    fn check_in(&mut self, ticket: Ticket) -> Result<CheckInResponse, WaitingRoomError>;

    /// This is the function the user should call when they want to leave the waiting room.
    /// If the ticket is valid, a pass is returned that the user can use to access the resource.
    /// If the ticket is invalid, an error is returned instead.
    fn leave(&mut self, ticket: Ticket) -> Result<Pass, WaitingRoomError>;

    /// This function is used to validate whether a pass is valid. If it is valid, it is refreshed and returned.
    /// If it is invalid, an error is returned instead.
    fn validate_and_refresh_pass(&mut self, pass: Pass) -> Result<Pass, WaitingRoomError>;
}

/// Returned by the [`WaitingRoomUserTriggered::check_in`] function.
#[derive(Debug)]
pub struct CheckInResponse {
    /// This is the refreshed ticket with the updated refresh and expiry times.
    pub new_ticket: Ticket,
    /// This is the position estimate of the user in the queue. This estimate
    /// is never lower than the previous estimate in the ticket.
    /// If the estimate is 0, the user is at the front of the queue and should
    /// call [`WaitingRoomUserTriggered::leave`].
    pub position_estimate: usize,
}

/// These functions are able to be triggered by timers.
/// For proper operation of the waiting room, all of these functions
/// need to be called periodically.
pub trait WaitingRoomTimerTriggered {
    /// This function is used to clean up expired tickets and passes.
    /// When a pass is invalidated, a new user is automatically let in.
    /// This function should be called somewhat regularly, eg. every 10 seconds.
    fn cleanup(&mut self) -> Result<(), WaitingRoomError>;

    /// This function is used to ensure that the correct number of users are on the site.
    /// If there are less than the minimum number of users, more users are let in.
    /// If there are more than the maximum number of users, users are not let in a number of times.
    /// This function should be called every Xth second, at roughly the same time on all nodes (preferably within a single network request's latency).
    fn eviction(&mut self) -> Result<(), WaitingRoomError>;

    /// Calling this function will trigger the fault detection mechanism.
    /// This function should be triggered frequently (eg. every 100ms). It can handle being called too frequently with little overhead.
    /// The fault detection mechanism is used to detect when a node has failed and remove it from the network.
    fn fault_detection(&mut self) -> Result<(), WaitingRoomError>;
}

/// These functions are able to be triggered by messages from nodes,
/// either the same node or a different node.
/// These functions are only implemented for waiting rooms that support
/// multi-node operation.
pub trait WaitingRoomMessageTriggered {
    /// Receive and process a single message from the network.
    fn receive_message(&mut self) -> Result<bool, WaitingRoomError> {
        unimplemented!(
            "This waiting room was not expecting to receive any messages, but got one anyway."
        )
    }
}

/// In cases where both a ticket and pass are supported, this enum is used to specify which one is.
pub enum Identification {
    Ticket(Ticket),
    Pass(Pass),
}
