use waitingroom_core::{pass::Pass, ticket::Ticket, time::Time};

#[derive(Debug)]
pub struct User {
    next_action_time: Time,
    next_action: UserAction,
    ticket: Option<Ticket>,
    pass: Option<Pass>,
    user_state: UserState,
}

impl User {
    pub fn new_refreshing(ticket: Ticket) -> Self {
        Self {
            next_action_time: ticket.next_refresh_time,
            next_action: UserAction::Refresh,
            ticket: Some(ticket),
            pass: None,
            user_state: UserState::InQueue,
        }
    }

    pub fn refresh_ticket(&mut self, position: usize, new_ticket: Ticket) {
        self.ticket = Some(new_ticket);
        if position == 0 {
            // TODO: Add some randomness to this.
            self.next_action_time += 1; // We want to leave immediately if we are first in line.
            self.next_action = UserAction::Leave;
        } else {
            // TODO: Add a small chance of abandoning, and some randomness for when to check in.
            self.next_action_time = new_ticket.next_refresh_time;
            self.next_action = UserAction::Refresh;
        }
    }

    pub fn should_action(&self, time: Time) -> bool {
        time >= self.next_action_time
    }

    pub fn get_action(&self) -> UserAction {
        self.next_action
    }

    pub fn take_ticket(&mut self) -> Ticket {
        self.ticket.take().unwrap()
    }

    pub fn set_pass(&mut self, pass: Pass) {
        self.user_state = UserState::OnSite;
        self.next_action = UserAction::Done;
        self.pass = Some(pass);
    }

    pub fn abandon(&mut self) {
        self.next_action_time = Time::MAX; // We will never take another action.
        self.user_state = UserState::AbandonedQueue;
    }

    pub fn start_refreshing(&mut self) {
        self.next_action = UserAction::Refresh;
        self.next_action_time = Time::MIN;
    }

    pub fn get_eviction_time(&self) -> Option<Time> {
        self.pass.as_ref().map(|pass| pass.eviction_time)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum UserAction {
    Refresh,
    Leave,
    Done,
}

#[derive(Debug, Clone, Copy)]
pub enum UserState {
    InQueue,
    OnSite,
    AbandonedQueue,
}
