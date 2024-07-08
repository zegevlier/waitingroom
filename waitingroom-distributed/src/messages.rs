use waitingroom_core::{time::Time, NodeId};
use waitingroom_spanning_trees::SpanningTree;

use crate::weight_table::Weight;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum NodeToNodeMessage {
    QPIDUpdateMessage {
        weight: Weight,
        updated_iteration: u64,
    },
    QPIDDeleteMin,
    QPIDFindRootMessage {
        weight: Weight,
        updated_iteration: u64,
        last_eviction: Time,
    },
    CountRequest(Time),
    CountResponse {
        iteration: Time,
        queue_count: usize,
        on_site_count: usize,
    },
    FaultDetectionRequest(Time),
    FaultDetectionResponse(Time),
    NodeAdded(NodeId, SpanningTree, usize),
    NodeRemoved(NodeId, SpanningTree, usize),
    TreeRestructure(SpanningTree, usize),
    NodeJoin(NodeId),
}
