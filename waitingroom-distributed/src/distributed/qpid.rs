use waitingroom_core::{
    network::{Network, NetworkHandle},
    random::RandomProvider,
    ticket::TicketType,
    time::{Time, TimeProvider},
    NodeId, WaitingRoomError, WaitingRoomTimerTriggered,
};

use crate::{messages::NodeToNodeMessage, weight_table::Weight, DistributedWaitingRoom};

/// The buffer time is used to ensure we are a bit more lenient on the eviction interval.
/// We don't want to evict too often.
const BUFFER_TIME: Time = 10;

impl<T, R, N> DistributedWaitingRoom<T, R, N>
where
    T: TimeProvider,
    R: RandomProvider,
    N: Network<NodeToNodeMessage>,
{
    /// For this, and all other QPID functions, see QPID paper and thesis for more information.
    /// Algorithm 1 - insert
    pub(super) fn qpid_insert(&mut self, weight: Weight) -> Result<(), WaitingRoomError> {
        if self.qpid_parent.is_none() {
            return Err(WaitingRoomError::QPIDNotInitialized);
        }

        let old_w_v_parent_v = self
            .qpid_weight_table
            .compute_weight(self.qpid_parent.unwrap());
        self.qpid_weight_table.set(self.node_id, weight, 0);

        if self.qpid_parent.unwrap() != self.node_id {
            let new_w_v_parent_v = self
                .qpid_weight_table
                .compute_weight(self.qpid_parent.unwrap());
            if new_w_v_parent_v != old_w_v_parent_v {
                self.send_qpid_update(self.qpid_parent.unwrap(), new_w_v_parent_v)?;
            }
        } else {
            self.broadcast_latest_values()?;
        }

        Ok(())
    }

    pub(super) fn send_qpid_update(
        &mut self,
        node: NodeId,
        weight: Weight,
    ) -> Result<(), WaitingRoomError> {
        let updated_iteration = self.get_update_iteration(node);
        self.network_handle.send_message(
            node,
            NodeToNodeMessage::QPIDUpdateMessage {
                weight,
                updated_iteration,
            },
        )?;
        self.qpid_last_update_values
            .iter_mut()
            .find(|(id, _)| *id == node)
            .map(|(_, value)| {
                *value = weight;
            })
            .unwrap_or_else(|| {
                self.qpid_last_update_values.push((node, weight));
            });
        Ok(())
    }

    /// For this, and all other QPID functions, see QPID paper and thesis for more information.
    /// Algorithm 2 - update
    pub(super) fn qpid_handle_update(
        &mut self,
        from_node: NodeId,
        weight: Weight,
        update_iteration: u64,
    ) -> Result<(), WaitingRoomError> {
        log::info!("[NODE {}] handle update", self.node_id);

        let mut old_w_v_parent_v = if let Some(qpid_parent) = self.qpid_parent {
            Some(self.qpid_weight_table.compute_weight(qpid_parent))
        } else {
            None
        };

        self.qpid_weight_table
            .set(from_node, weight, update_iteration);

        if self.qpid_parent.is_none() {
            // QPID is uninitialized. This is either when a network change happened, or when we haven't initialized at all yet.
            // If this message completes our QPID table, we can let it through. Otherwise, we need to wait for more messages.

            if !self.heuristic_set_qpid_parent() {
                // We didn't have enough information to set the QPID parent, so we can't do anything yet.
                return Ok(());
            }

            old_w_v_parent_v = Some(
                self.qpid_weight_table
                    .compute_weight(self.qpid_parent.unwrap()),
            );

            // If we found our parent here, we need to send some more updates, since we didn't have the most up to date info before.
            for node in self.qpid_weight_table.get_all_neighbours() {
                if node == self.qpid_parent.unwrap() || node == self.node_id {
                    continue;
                }
                let updated_weight = self.qpid_weight_table.compute_weight(node);
                self.send_qpid_update(node, updated_weight)?;
            }
        }

        if self.qpid_parent.unwrap() == self.node_id {
            if weight < self.qpid_weight_table.get_weight(self.node_id).unwrap() {
                self.qpid_parent = Some(from_node);
                let w_v_u = self.qpid_weight_table.compute_weight(from_node);
                let updated_iteration = self.get_update_iteration(from_node);
                self.network_handle
                    .send_message(
                        from_node,
                        NodeToNodeMessage::QPIDFindRootMessage {
                            weight: w_v_u,
                            last_eviction: self.count_iteration,
                            updated_iteration,
                        },
                    )
                    .unwrap()
            }
        } else {
            let new_w_v_parent_v = self
                .qpid_weight_table
                .compute_weight(self.qpid_parent.unwrap());
            if new_w_v_parent_v != old_w_v_parent_v.unwrap() {
                self.send_qpid_update(self.qpid_parent.unwrap(), new_w_v_parent_v)?;
            }
        }

        Ok(())
    }

    /// For this, and all other QPID functions, see QPID paper and thesis for more information.
    /// Algorithm 3 - deleteMin
    pub(super) fn qpid_delete_min(&mut self) -> Result<(), WaitingRoomError> {
        log::info!("[NODE {}] QPID delete min", self.node_id);
        if self.qpid_parent.is_none() {
            log::warn!("QPID not initialized");
            return Ok(());
        }

        if self.qpid_parent.unwrap() != self.node_id {
            self.network_handle
                .send_message(self.qpid_parent.unwrap(), NodeToNodeMessage::QPIDDeleteMin)
                .unwrap();
            return Ok(());
        }

        if self.local_queue.is_empty() {
            return Ok(());
        }

        let mut ticket = self.dequeue().unwrap();

        // Update current QPID weight
        match self.local_queue.peek() {
            Some(next_ticket) => {
                self.qpid_weight_table.set(
                    self.node_id,
                    Weight::new(next_ticket.join_time, next_ticket.identifier, self.node_id),
                    0,
                );
            }
            None => {
                self.qpid_weight_table.set(
                    self.node_id,
                    Weight::new(Time::MAX, 0, self.node_id),
                    0,
                );
            }
        }

        if self.qpid_weight_table.any_not_max() {
            let new_parent = self.qpid_weight_table.get_smallest().unwrap();
            self.qpid_parent = Some(new_parent);
            if new_parent != self.node_id {
                let updated_weight = self.qpid_weight_table.compute_weight(new_parent);
                let updated_iteration = self.get_update_iteration(new_parent);
                self.network_handle
                    .send_message(
                        new_parent,
                        NodeToNodeMessage::QPIDFindRootMessage {
                            weight: updated_weight,
                            last_eviction: self.count_iteration,
                            updated_iteration,
                        },
                    )
                    .unwrap();
            } else {
                self.broadcast_latest_values()?;
            }
        }

        match ticket.ticket_type {
            TicketType::Normal => {
                ticket.set_eviction_time(self.time_provider.get_now_time());
                self.local_queue_leaving_list.push(ticket);
                metrics::gauge!(
                    "waitingroom.to_let_in_count",
                    "node_id" => self.node_id.to_string()
                )
                .increment(1);
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

    fn broadcast_latest_values(&mut self) -> Result<(), WaitingRoomError> {
        for node in self.qpid_weight_table.get_true_neighbours() {
            if node == self.node_id {
                continue;
            }

            let weight = self.qpid_weight_table.compute_weight(node);

            // If we've already sent the latest update, we don't need to do that here
            if let Some((_, last_update)) = self
                .qpid_last_update_values
                .iter()
                .find(|(id, _)| *id == node)
            {
                if last_update == &weight {
                    continue;
                }
            }

            self.send_qpid_update(node, weight)?;
        }
        Ok(())
    }

    /// For this, and all other QPID functions, see QPID paper and thesis for more information.
    /// Algorithm 4 - findRoot
    pub(super) fn qpid_handle_find_root(
        &mut self,
        from_node: NodeId,
        weight: Weight,
        last_eviction: Time,
        updated_iteration: u64,
    ) -> Result<(), WaitingRoomError> {
        log::info!("[NODE {}] handle find root", self.node_id);

        if self.qpid_parent.is_none() {
            log::warn!("QPID not initialized");
            return Ok(());
            // return Err(WaitingRoomError::QPIDNotInitialized);
        }

        self.qpid_weight_table
            .set(from_node, weight, updated_iteration);

        if self.qpid_weight_table.any_not_max() {
            log::debug!(
                "[NODE {}] Initializing QPID with values from weight table in findRoot",
                self.node_id
            );
            self.qpid_parent = Some(self.qpid_weight_table.get_smallest().unwrap());
        } else {
            log::debug!(
                "[NODE {}] Initializing QPID with values from spanning tree in findRoot",
                self.node_id
            );
            // Otherwise, the value that will lead us to the lowest node ID is our parent.
            self.qpid_parent = Some(self.spanning_tree.towards_lowest_id(self.node_id));
        }

        if self.qpid_parent.unwrap() != self.node_id {
            let w_v_parent_v = self
                .qpid_weight_table
                .compute_weight(self.qpid_parent.unwrap());

            let updated_iteration = self.get_update_iteration(self.qpid_parent.unwrap());
            self.network_handle
                .send_message(
                    self.qpid_parent.unwrap(),
                    NodeToNodeMessage::QPIDFindRootMessage {
                        weight: w_v_parent_v,
                        last_eviction,
                        updated_iteration,
                    },
                )
                .unwrap()
        } else {
            // We are the new parent. This is not part of regular QPID.
            // We need to trigger a new eviction if the last eviction was too long ago.
            let now = self.time_provider.get_now_time();
            if now - last_eviction > self.settings.eviction_interval + BUFFER_TIME {
                // self.eviction()?;
            }
        }
        Ok(())
    }

    pub(crate) fn heuristic_set_qpid_parent(&mut self) -> bool {
        // if self.spanning_tree.get_node_list().len() == 1 {
        //     // We are the only node in the network. We are our own parent.
        //     self.qpid_parent = Some(self.node_id);
        //     return true;
        // }

        // If the qpid weight table does not contain all the spanning tree neighbours
        // or if the spanning tree is empty, we can't initialize QPID yet.
        let mut all_contained = true;
        for node in self.spanning_tree.get_node(self.node_id).unwrap() {
            if self.qpid_weight_table.get_weight(*node).is_none() {
                all_contained = false;
                break;
            }
        }

        if !all_contained
            || self
                .spanning_tree
                .get_node(self.node_id)
                .unwrap()
                .is_empty()
        // This is when we haven't even gotten the first spanning tree yet.
        {
            log::debug!(
                "[NODE {}] QPID not initialized yet. Waiting for more messages",
                self.node_id
            );
            // We don't have all the information yet. We need to wait for more messages.
            return false;
        }

        log::debug!("[NODE {}] Heuristic set QPID parent", self.node_id);
        // We have all the information we need. We can initialize QPID.
        if self.qpid_weight_table.any_not_max() {
            log::debug!(
                "[NODE {}] Initializing QPID with values from weight table",
                self.node_id
            );
            self.qpid_parent = Some(self.qpid_weight_table.get_smallest().unwrap());
        } else {
            log::debug!(
                "[NODE {}] Initializing QPID with values from spanning tree",
                self.node_id
            );
            // Otherwise, the value that will lead us to the lowest node ID is our parent.
            self.qpid_parent = Some(self.spanning_tree.towards_lowest_id(self.node_id));
        }

        // We've found a new parent, if we needed to send a find root, we do it here.
        if self.should_send_find_root {
            if self.qpid_parent.unwrap() == self.node_id {
                // We shouldn't send a find root to ourselves.
                return true;
            }
            let w_v_parent_v = self
                .qpid_weight_table
                .compute_weight(self.qpid_parent.unwrap());
            let updated_iteration = self.get_update_iteration(self.qpid_parent.unwrap());
            self.network_handle
                .send_message(
                    self.qpid_parent.unwrap(),
                    NodeToNodeMessage::QPIDFindRootMessage {
                        weight: w_v_parent_v,
                        last_eviction: self.count_iteration,
                        updated_iteration,
                    },
                )
                .unwrap();
            self.should_send_find_root = false;
        }
        true
    }

    /// This returns the current update iteration, and increases it by one.
    pub(crate) fn get_update_iteration(&mut self, node_id: NodeId) -> u64 {
        self.qpid_update_iterations
            .iter_mut()
            .find(|(id, _)| *id == node_id)
            .map(|(_, iteration)| {
                *iteration += 1;
                *iteration
            })
            .unwrap_or_else(|| {
                self.qpid_update_iterations.push((node_id, 0));
                0
            })
    }
}
