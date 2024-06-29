use waitingroom_core::{pass::Pass, ticket::Ticket, time::Time};

#[derive(Debug, Clone)]
pub struct UserBehaviour {
    pub abandon_odds: u64,
    pub pass_refresh_odds: u64,
}

#[derive(Debug)]
pub struct User {
    next_action_time: Time,
    state: UserState,
}

#[derive(Debug, Clone, strum::EnumIs)]
pub enum UserState {
    Joining,
    InQueue {
        ticket: Ticket,
        next_action: QueueAction,
    },
    OnSite {
        pass: Pass,
    },
    Done {
        joined_at: Time,
        evicted_at: Time,
    },
    Abandoned {
        joined_at: Time,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueAction {
    Refreshing,
    Abandoning,
    Leaving,
}

impl User {
    pub fn should_action(&self, now: Time) -> bool {
        now >= self.next_action_time
    }

    pub fn new() -> Self {
        Self {
            next_action_time: 0,
            state: UserState::Joining,
        }
    }

    pub fn state(&self) -> &UserState {
        &self.state
    }

    pub fn join(&mut self, ticket: Ticket) {
        assert!(self.state.is_joining());

        self.next_action_time = ticket.next_refresh_time;
        self.state = UserState::InQueue {
            ticket,
            next_action: QueueAction::Refreshing,
        };
    }

    pub fn next_action(&self) -> QueueAction {
        match &self.state {
            UserState::InQueue { next_action, .. } => *next_action,
            _ => panic!("User is not in queue, but next action was requested"),
        }
    }

    pub fn refresh_ticket(&mut self, new_ticket: Ticket, position_estimate: usize) {
        assert_eq!(self.next_action(), QueueAction::Refreshing);
        if position_estimate == 0 {
            // We're at the front of the queue.
            self.next_action_time = 0; // We want to leave the queue ASAP
            self.state = UserState::InQueue {
                ticket: new_ticket,
                next_action: QueueAction::Leaving,
            };
            return;
        }

        self.next_action_time = new_ticket.next_refresh_time;
        self.state = UserState::InQueue {
            ticket: new_ticket,
            next_action: QueueAction::Refreshing,
        };
    }

    pub fn return_to_refreshing(&mut self) {
        assert_eq!(self.next_action(), QueueAction::Leaving);
        if let UserState::InQueue { ticket, .. } = &self.state {
            // This will always be the case
            self.next_action_time = ticket.next_refresh_time;
            self.state = UserState::InQueue {
                ticket: *ticket,
                next_action: QueueAction::Refreshing,
            };
        }
    }

    pub fn leave(&mut self, pass: Pass) {
        assert_eq!(self.next_action(), QueueAction::Leaving);
        self.state = UserState::OnSite { pass };
        self.next_action_time = Time::MAX; // TODO: Add refreshing pass
    }

    pub fn abandon(&mut self) {
        assert_eq!(self.next_action(), QueueAction::Abandoning);
        let joined_at = match &self.state {
            UserState::InQueue { ticket, .. } => ticket.join_time,
            _ => panic!("User is not in queue, but next action was requested"),
        };
        self.state = UserState::Abandoned { joined_at };
        self.next_action_time = Time::MAX;
    }

    pub fn refresh_pass(&mut self, new_pass: Pass) {
        assert!(self.state.is_on_site());
        self.state = UserState::OnSite { pass: new_pass };
    }

    pub fn finish(&mut self) {
        self.state = match self.state {
            UserState::OnSite { pass } => UserState::Done {
                joined_at: pass.queue_join_time,
                evicted_at: pass.eviction_time,
            },
            _ => panic!("User is not on site, but finish was requested"),
        };

        self.next_action_time = Time::MAX;
    }

    pub fn get_join_time(&self) -> Option<Time> {
        match &self.state {
            UserState::InQueue { ticket, .. } => Some(ticket.join_time),
            UserState::OnSite { pass } => Some(pass.queue_join_time),
            UserState::Done { joined_at, .. } => Some(*joined_at),
            UserState::Abandoned { joined_at } => Some(*joined_at),
            _ => None,
        }
    }

    pub fn get_eviction_time(&self) -> Option<Time> {
        match &self.state {
            UserState::OnSite { pass } => Some(pass.eviction_time),
            UserState::Done { evicted_at, .. } => Some(*evicted_at),
            _ => None,
        }
    }
}
