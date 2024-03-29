#[derive(Debug)]
pub enum WaitingRoomError {
    TicketExpired,
    TicketNotInQueue,
    TicketAtWrongNode,
    TicketCannotLeaveYet,
    PassExpired,
    PassNotInList,
}

impl std::fmt::Display for WaitingRoomError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WaitingRoomError::TicketExpired => write!(f, "Ticket expired"),
            WaitingRoomError::TicketNotInQueue => write!(f, "Ticket not in queue"),
            WaitingRoomError::TicketAtWrongNode => write!(f, "Ticket at wrong node"),
            WaitingRoomError::TicketCannotLeaveYet => write!(f, "Ticket cannot leave yet"),
            WaitingRoomError::PassExpired => write!(f, "Pass expired"),
            WaitingRoomError::PassNotInList => write!(f, "Pass not in list"),
        }
    }
}
