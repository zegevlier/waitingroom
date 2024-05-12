use waitingroom_core::{
    metrics,
    network::{Network, NetworkHandle},
    random::RandomProvider,
    ticket::TicketType,
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
    /// For this, and all other QPID functions, see QPID paper and thesis for more information.
    /// Algorithm 1 - insert
    pub(super) fn qpid_insert(&mut self, weight: Time) {
        self.qpid_weight_table.set(self.node_id, weight);
        if let Some(qpid_parent) = self.qpid_parent {
            if qpid_parent == self.node_id {
                return;
            }
            let updated_weight = self.qpid_weight_table.compute_weight(qpid_parent);
            if updated_weight != self.qpid_weight_table.get(qpid_parent).unwrap() {
                self.qpid_weight_table.set(qpid_parent, updated_weight);
                self.network_handle
                    .send_message(
                        qpid_parent,
                        NodeToNodeMessage::QPIDUpdateMessage(updated_weight),
                    )
                    .unwrap();
            }
        } else {
            log::warn!("QPID parent is None when trying to insert");
        }
    }

    /// For this, and all other QPID functions, see QPID paper and thesis for more information.
    /// Algorithm 2 - update
    pub(super) fn qpid_handle_update(
        &mut self,
        from_node: NodeId,
        weight: Time,
    ) -> Result<(), WaitingRoomError> {
        log::info!("[NODE {}] handle update", self.node_id);
        if self.qpid_parent.is_none() {
            return Err(WaitingRoomError::QPIDNotInitialized);
        }
        let qpid_parent = self.qpid_parent.unwrap();

        self.qpid_weight_table.set(from_node, weight);
        if self.qpid_parent == Some(self.node_id) {
            if weight < self.qpid_weight_table.get(self.node_id).unwrap() {
                self.qpid_parent = Some(from_node);
                let updated_weight = self.qpid_weight_table.compute_weight(from_node);
                self.qpid_weight_table.set(from_node, updated_weight);
                self.network_handle
                    .send_message(
                        from_node,
                        NodeToNodeMessage::QPIDFindRootMessage(updated_weight),
                    )
                    .unwrap()
            }
        } else {
            let updated_weight = self.qpid_weight_table.compute_weight(qpid_parent);
            if updated_weight != self.qpid_weight_table.get(qpid_parent).unwrap() {
                self.qpid_weight_table.set(qpid_parent, updated_weight);
                self.network_handle
                    .send_message(
                        qpid_parent,
                        NodeToNodeMessage::QPIDUpdateMessage(updated_weight),
                    )
                    .unwrap()
            }
        }

        Ok(())
    }

    /// For this, and all other QPID functions, see QPID paper and thesis for more information.
    /// Algorithm 3 - deleteMin
    pub(super) fn qpid_delete_min(&mut self) -> Result<(), WaitingRoomError> {
        log::info!("[NODE {}] QPID delete min", self.node_id);
        if self.qpid_parent.is_none() {
            return Err(WaitingRoomError::QPIDNotInitialized);
        }
        let qpid_parent = self.qpid_parent.unwrap();

        if qpid_parent != self.node_id {
            self.network_handle
                .send_message(qpid_parent, NodeToNodeMessage::QPIDDeleteMin)
                .unwrap();
            return Ok(());
        }

        if self.local_queue.is_empty() {
            return Ok(());
        }

        let ticket = self.dequeue().unwrap();

        // Update current QPID weight
        match self.local_queue.peek() {
            Some(next_ticket) => {
                self.qpid_weight_table
                    .set(self.node_id, next_ticket.join_time);
            }
            None => {
                self.qpid_weight_table.set(self.node_id, Time::MAX);
            }
        }

        if self.qpid_weight_table.any_not_max() {
            let new_parent = self.qpid_weight_table.get_smallest().unwrap();
            self.qpid_parent = Some(new_parent);
            if new_parent != self.node_id {
                let updated_weight = self.qpid_weight_table.compute_weight(new_parent);
                self.qpid_weight_table.set(new_parent, updated_weight);
                self.network_handle
                    .send_message(
                        new_parent,
                        NodeToNodeMessage::QPIDFindRootMessage(updated_weight),
                    )
                    .unwrap();
            }
        }

        match ticket.ticket_type {
            TicketType::Normal => {
                self.local_queue_leaving_list.push(ticket);
                metrics::waitingroom::to_be_let_in_count(self.node_id).inc();
            }
            TicketType::Drain => {
                // This ticket is a dummy ticket. We shouldn't do anything with it.
            }
            TicketType::Skip => {
                // For this ticket, we need to take someone else out of the queue.
                self.qpid_delete_min()?;
            }
        }

        Ok(())
    }

    /// For this, and all other QPID functions, see QPID paper and thesis for more information.
    /// Algorithm 4 - findRoot
    pub(super) fn qpid_handle_find_root(
        &mut self,
        from_node: NodeId,
        weight: Time,
    ) -> Result<(), WaitingRoomError> {
        log::info!("[NODE {}] handle find root", self.node_id);

        if self.qpid_parent.is_none() {
            return Err(WaitingRoomError::QPIDNotInitialized);
        }

        let qpid_parent = self.qpid_parent.unwrap();
        self.qpid_weight_table.set(from_node, weight);
        self.qpid_parent = self.qpid_weight_table.get_smallest();
        if qpid_parent != self.node_id {
            let updated_weight = self.qpid_weight_table.compute_weight(qpid_parent);
            self.qpid_weight_table.set(qpid_parent, updated_weight);
            self.network_handle
                .send_message(
                    from_node,
                    NodeToNodeMessage::QPIDUpdateMessage(updated_weight),
                )
                .unwrap()
        }
        Ok(())
    }
}
