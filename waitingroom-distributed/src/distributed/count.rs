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
    /// The count operations are used to determine the total number of users on the site on the entire network.
    /// This initiates a count request, which is then propagated through the network.
    /// See thesis for more information.
    pub(super) fn count_request(
        &mut self,
        from_node: NodeId,
        count_iteration: Time,
    ) -> Result<(), WaitingRoomError> {
        log::info!("[NODE {}] count request", self.node_id);
        if count_iteration <= self.count_iteration {
            // We've already participated in a count iteration that is higher than this one.
            // We don't need to respond.
            log::debug!(
                "[NODE {}] count request from {} with count {} is lower than current count {}",
                self.node_id,
                from_node,
                count_iteration,
                self.count_iteration
            );
            return Ok(());
        }

        self.count_iteration = count_iteration;
        self.count_parent = Some(from_node);
        self.count_responses.clear();

        // If we have any neighbours, we need to ask them to participate in the count before we can respond.
        if self.qpid_weight_table.neighbour_count() > 1 || self.node_id == from_node {
            for node_id in &self.qpid_weight_table.all_neighbours() {
                if *node_id != from_node {
                    self.network_handle
                        .send_message(*node_id, NodeToNodeMessage::CountRequest(count_iteration))?;
                }
            }
        } else {
            // If we don't have any neighbours, we can respond immediately.
            if self.node_id == from_node {
                self.count_response(from_node, count_iteration, 0)?;
            } else {
                self.network_handle.send_message(
                    from_node,
                    NodeToNodeMessage::CountResponse(count_iteration, self.get_on_site_count()),
                )?;
            }
        }

        Ok(())
    }

    /// See thesis for more information.
    pub(super) fn count_response(
        &mut self,
        from_node: NodeId,
        count_iteration: Time,
        count: usize,
    ) -> Result<(), WaitingRoomError> {
        log::info!(
            "[NODE {}] count response fr: {} it: {} c: {}",
            self.node_id,
            from_node,
            count_iteration,
            count
        );
        if count_iteration != self.count_iteration {
            // This message isn't part of the current count iteration. Ignore it.
            log::debug!(
                "[NODE {}] count response from {} with count {} is not for current count {}",
                self.node_id,
                from_node,
                count_iteration,
                self.count_iteration
            );
            return Ok(());
        }

        self.count_responses.push((from_node, count));

        if self.count_responses.len() >= self.qpid_weight_table.neighbour_count() {
            // We have received all responses.
            let others_count = self
                .count_responses
                .iter()
                .map(|(_, count)| *count)
                .sum::<usize>();

            let own_count = self.get_on_site_count();
            let total_count = others_count + own_count;

            if Some(self.node_id) == self.count_parent {
                // We are the count root, so we need to let users out of the queue.
                log::debug!(
                    "[NODE {}] count root with total count {}",
                    self.node_id,
                    total_count
                );
                self.ensure_correct_site_count(total_count)?;
            } else {
                // We are not the count parent node, so we need to send our total count to the parent node.
                self.network_handle.send_message(
                    self.count_parent.unwrap(),
                    NodeToNodeMessage::CountResponse(count_iteration, total_count),
                )?;
            }
        }

        Ok(())
    }

    /// Get the number of users currently on the site, including the ones that are about to leave the queue.
    pub(super) fn get_on_site_count(&self) -> usize {
        self.local_on_site_list.len() + self.local_queue_leaving_list.len()
    }
}
