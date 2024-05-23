use waitingroom_core::{time::Time, NodeId};
use waitingroom_spanning_trees::SpanningTree;

#[derive(Debug, Clone)]
pub enum NodeToNodeMessage {
    QPIDUpdateMessage(Time),
    QPIDDeleteMin,
    QPIDFindRootMessage { weight: Time, last_eviction: Time },
    CountRequest(Time),
    CountResponse(Time, usize),
    FaultDetectionRequest(Time),
    FaultDetectionResponse(Time),
    NodeAdded(NodeId, SpanningTree, usize),
    NodeRemoved(NodeId, SpanningTree, usize),
    TreeRestructure(SpanningTree, usize),
    NodeJoin(NodeId),
}
