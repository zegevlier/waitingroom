use waitingroom_core::time::Time;

#[derive(Debug, Clone)]
pub enum NodeToNodeMessage {
    QPIDUpdateMessage(Time),
    QPIDDeleteMin,
    QPIDFindRootMessage { weight: Time, last_eviction: Time },
    CountRequest(Time),
    CountResponse(Time, usize),
    FaultDetectionRequest(Time),
    FaultDetectionResponse(Time),
}
