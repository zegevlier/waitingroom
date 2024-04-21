use waitingroom_core::time::Time;

#[derive(Debug, Clone)]
pub enum NodeToNodeMessage {
    QPIDUpdateMessage(Time),
    QPIDDeleteMin,
    QPIDFindRootMessage(Time),
}
