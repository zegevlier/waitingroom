use std::{fmt::Debug, ops::DerefMut, sync::{Arc, Mutex}};

/// The type for time values. This is the number of milliseconds since the UNIX epoch.
pub type Time = u128;

pub trait TimeProvider: Debug {
    /// This utility function is used to get the current time in milliseconds since the UNIX epoch.
    /// This is used to set the join time, refresh time and expiry time of tickets and passes.
    fn get_now_time(&self) -> Time;
}

#[derive(Debug)]
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

#[derive(Debug, Clone)]
pub struct DummyTimeProvider {
    // This is only used in a single threaded context, but for logging purposes, we need to use an Arc.
    // Not ideal, but it shouldn't impact performance much. Old implementation using Rc<Cell<Time>> is
    // left in the comments for reference.
    time: Arc<Mutex<Time>>,
}

impl DummyTimeProvider {
    pub fn new() -> Self {
        DummyTimeProvider {
            time: Arc::new(Mutex::new(Time::default())),
        }
    }

    pub fn increase_by(&self, amount: Time) {
        log::debug!("Increasing dummy time by {}", amount);
        // self.time.set((*self.time).get() + amount);
        *self.time.lock().unwrap().deref_mut() += amount;
    }
}

impl Default for DummyTimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeProvider for DummyTimeProvider {
    fn get_now_time(&self) -> Time {
        // self.time.get()
        *self.time.lock().unwrap()
    }
}
