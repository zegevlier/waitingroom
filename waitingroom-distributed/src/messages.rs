use waitingroom_core::time::Time;

#[derive(Debug, Clone)]
pub enum NodeToNodeMessage {
    QPIDUpdateMessage(Time),
    QPIDDeleteMin,
    QPIDFindRootMessage(Time),
    CountRequest(Time),
    CountResponse(Time, usize),
    FaultDetectionRequest(Time),
    FaultDetectionResponse(Time),
}
