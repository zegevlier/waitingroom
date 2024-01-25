use foundations::telemetry::metrics::{metrics, Gauge};

#[metrics]
pub(crate) mod waitingroom_basic {
    pub(crate) fn in_queue_count(node_id: u64) -> Gauge;
}
