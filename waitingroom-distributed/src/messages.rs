use waitingroom_core::{time::Time, NodeId};
use waitingroom_spanning_trees::SpanningTree;

#[derive(Debug, Clone)]
pub enum NodeToNodeMessage {
    QPIDUpdateMessage {
        weight: Time,
        updated_iteration: u64,
    },
    QPIDDeleteMin,
    QPIDFindRootMessage {
        weight: Time,
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
