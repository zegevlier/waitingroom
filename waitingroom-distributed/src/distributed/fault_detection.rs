use waitingroom_core::{
    network::{Network, NetworkHandle},
    random::RandomProvider,
    time::{Time, TimeProvider},
    NodeId, WaitingRoomError,
};

use crate::{messages::NodeToNodeMessage, DistributedWaitingRoom};

impl<T, R, N> DistributedWaitingRoom<T, R, N>
where
    T: TimeProvider,
    R: RandomProvider,
    N: Network<NodeToNodeMessage>,
{
    pub(super) fn fault_detection_request(
        &mut self,
        from_node: NodeId,
        check_id: Time,
    ) -> Result<(), WaitingRoomError> {
        log::info!(
            "[NODE {}] fault detection request from {}",
            self.node_id,
            from_node
        );

        self.network_handle.send_message(
            from_node,
            NodeToNodeMessage::FaultDetectionResponse(check_id),
        )?;
        Ok(())
    }

    pub(super) fn fault_detection_response(
        &mut self,
        from_node: NodeId,
        check_id: Time,
    ) -> Result<(), WaitingRoomError> {
        // When we get a response, and the response is to the current check, the check succeeded and
        // we can mark the check as such by setting the node to None.
        if let Some(node) = self.fd_last_check_node {
            if node == from_node && check_id == self.fd_last_check_time {
                log::info!(
                    "[NODE {}] fault detection response from {}",
                    self.node_id,
                    from_node
                );
                self.fd_last_check_node = None;
            }
        }

        Ok(())
    }
}
