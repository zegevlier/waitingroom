#[derive(Debug)]
pub enum WaitingRoomError {
    TicketExpired,
    TicketNotInQueue,
    TicketInvalid,
    TicketAtWrongNode,
    TicketCannotLeaveYet,
    PassExpired,
}
