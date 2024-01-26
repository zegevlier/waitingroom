use foundations::telemetry::metrics::{metrics, Gauge};

#[metrics]
pub(crate) mod waitingroom_basic {
    pub(crate) fn in_queue_count(node_id: u64) -> Gauge;

    pub(crate) fn to_be_let_in_count(node_id: u64) -> Gauge;

    pub(crate) fn on_site_count(node_id: u64) -> Gauge;
}
