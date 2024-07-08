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

            // This means that two counts have collided, which probably means we have two roots.
            self.failed_counts += 1;

            if self.failed_counts > 10 {
                // We've failed to respond to a count request too many times.
                // We need to restructure the tree.
                self.restructure_tree()?;
            }
        }

        self.count_iteration = count_iteration;
        self.count_parent = Some(from_node);
        self.count_responses.clear();

        // If we have any neighbours, we need to ask them to participate in the count before we can respond.
        if self.qpid_weight_table.neighbour_count() > 1 || (self.node_id == from_node && self.qpid_weight_table.neighbour_count() > 0){
            for node_id in &self.qpid_weight_table.get_true_neighbours() {
                if *node_id != from_node && *node_id != self.node_id {
                    self.network_handle
                        .send_message(*node_id, NodeToNodeMessage::CountRequest(count_iteration))?;
                }
            }
        } else {
            // If we don't have any neighbours, we can respond immediately.
            if self.node_id == from_node {
                // We add 0 here, since we add our own counts later
                self.count_response(from_node, count_iteration, 0, 0)?;
            } else {
                self.network_handle.send_message(
                    from_node,
                    NodeToNodeMessage::CountResponse {
                        iteration: count_iteration,
                        queue_count: self.in_queue_count(),
                        on_site_count: self.get_local_on_site_count(),
                    },
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
        queue_count: usize,
        on_site_count: usize,
    ) -> Result<(), WaitingRoomError> {
        log::info!(
            "[NODE {}] count response fr: {} it: {} q: {} s: {}",
            self.node_id,
            from_node,
            count_iteration,
            queue_count,
            on_site_count
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

        self.count_responses
            .push((from_node, queue_count, on_site_count));

        if self.count_responses.len()
            >= self
                .qpid_weight_table
                .get_true_neighbours()
                .iter()
                .filter(|n| **n != self.count_parent.unwrap() && **n != self.node_id)
                .count()
        {
            self.failed_counts = 0;
            // We have received all responses.
            let others_queue_count = self
                .count_responses
                .iter()
                .map(|(_, queue_count, _)| (queue_count))
                .sum::<usize>();

            let others_on_site_count = self
                .count_responses
                .iter()
                .map(|(_, _, on_site_count)| (on_site_count))
                .sum::<usize>();

            let total_queue_count = others_queue_count + self.in_queue_count();
            let total_on_site_count = others_on_site_count + self.get_local_on_site_count();

            if Some(self.node_id) == self.count_parent {
                // We are the count root, so we need to let users out of the queue.
                log::debug!(
                    "[NODE {}] count root with total count q: {} s: {}",
                    self.node_id,
                    total_queue_count,
                    total_on_site_count,
                );
                self.ensure_correct_site_count(total_queue_count, total_on_site_count)?;
            } else {
                // We are not the count parent node, so we need to send our total count to the parent node.
                self.network_handle.send_message(
                    self.count_parent.unwrap(),
                    NodeToNodeMessage::CountResponse {
                        iteration: count_iteration,
                        queue_count: total_queue_count,
                        on_site_count: total_on_site_count,
                    },
                )?;
            }
        }

        Ok(())
    }

    /// Get the number of users currently on the site, including the ones that are about to leave the queue.
    pub fn get_local_on_site_count(&self) -> usize {
        self.local_on_site_list.len() + self.local_queue_leaving_list.len()
    }

    pub fn in_queue_count(&self) -> usize {
        self.local_queue.len()
    }

    pub fn in_queue_leaving_count(&self) -> usize {
        self.local_queue_leaving_list.len()
    }
}
