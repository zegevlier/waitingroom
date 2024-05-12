use foundations::telemetry::metrics::{metrics, Gauge};

#[allow(clippy::empty_docs)] // Macro generated this error - I don't know why
#[metrics]
pub mod waitingroom {
    pub fn in_queue_count(node_id: usize) -> Gauge;

    pub fn to_be_let_in_count(node_id: usize) -> Gauge;

    pub fn on_site_count(node_id: usize) -> Gauge;
}
