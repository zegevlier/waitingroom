use foundations::telemetry::metrics::{metrics, Gauge};

#[metrics]
pub mod waitingroom {
    pub fn in_queue_count(node_id: u64) -> Gauge;

    pub fn to_be_let_in_count(node_id: u64) -> Gauge;

    pub fn on_site_count(node_id: u64) -> Gauge;
}
