use std::sync::{Arc, Mutex};

use crate::Time;

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
    time: Arc<Mutex<Time>>,
}

impl DummyTimeProvider {
    pub fn new() -> Self {
        DummyTimeProvider {
            time: Arc::new(Mutex::new(Time::default())),
        }
    }

    pub fn increase_by(&self, amount: Time) {
        let mut time = self.time.lock().unwrap();
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
        *self.time.lock().unwrap()
    }
}
