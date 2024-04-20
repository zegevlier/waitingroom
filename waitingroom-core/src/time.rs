use std::{cell::RefCell, rc::Rc};

/// The type for time values. This is the number of milliseconds since the UNIX epoch.
pub type Time = u128;

pub trait TimeProvider {
    /// This utility function is used to get the current time in milliseconds since the UNIX epoch.
    /// This is used to set the join time, refresh time and expiry time of tickets and passes.
    fn get_now_time(&self) -> Time;
}

pub struct SystemTimeProvider;

impl SystemTimeProvider {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for SystemTimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeProvider for SystemTimeProvider {
    fn get_now_time(&self) -> Time {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    }
}

#[derive(Clone)]
pub struct DummyTimeProvider {
    time: Rc<RefCell<Time>>,
}

impl DummyTimeProvider {
    pub fn new() -> Self {
        DummyTimeProvider {
            time: Rc::new(RefCell::new(Time::default())),
        }
    }

    pub fn increase_by(&self, amount: Time) {
        let mut time = self.time.borrow_mut();
        *time += amount;
    }
}

impl Default for DummyTimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeProvider for DummyTimeProvider {
    fn get_now_time(&self) -> Time {
        *self.time.borrow()
    }
}
