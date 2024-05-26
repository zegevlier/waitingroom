use waitingroom_core::{
    network::{Network, NetworkHandle},
    random::RandomProvider,
    time::{Time, TimeProvider},
    NodeId, WaitingRoomError,
};
use waitingroom_spanning_trees::SpanningTree;

use crate::{messages::NodeToNodeMessage, DistributedWaitingRoom};

impl<T, R, N> DistributedWaitingRoom<T, R, N>
where
    T: TimeProvider,
    R: RandomProvider,
    N: Network<NodeToNodeMessage>,
{
    pub fn join_at(&mut self, at: NodeId) -> Result<(), WaitingRoomError> {
        log::debug!("[{}] Joining at {}", self.node_id, at);
        self.qpid_weight_table.set(self.node_id, Time::MAX, 0);
        self.network_handle
            .send_message(at, NodeToNodeMessage::NodeJoin(self.node_id))?;
        Ok(())
    }

    pub fn node_join_message(&mut self, node_id: NodeId) -> Result<(), WaitingRoomError> {
        log::debug!(
            "[{}] Received NodeJoin message from {}",
            self.node_id,
            node_id
        );
        self.add_node(node_id)
    }

    pub fn initialise_alone(&mut self) -> Result<(), WaitingRoomError> {
        log::debug!("[{}] Initialising alone", self.node_id);
        self.tree_iteration += 1;
        self.qpid_parent = Some(self.node_id);
        self.qpid_weight_table.set(self.node_id, Time::MAX, 0);
        Ok(())
    }

    pub fn add_node(&mut self, node_id: NodeId) -> Result<(), WaitingRoomError> {
        log::debug!("[{}] Adding node {}", self.node_id, node_id);
        self.network_members.push(node_id);
        let mut updated_tree = self.spanning_tree.clone();
        updated_tree.add_node(node_id);

        self.tree_iteration += 1;

        for member in &self.network_members {
            if *member != self.node_id {
                self.network_handle.send_message(
                    *member,
                    NodeToNodeMessage::NodeAdded(
                        node_id,
                        updated_tree.clone(),
                        self.tree_iteration,
                    ),
                )?;
            }
        }

        self.apply_new_tree(updated_tree)
    }

    pub(super) fn node_add_message(
        &mut self,
        node_id: NodeId,
        tree: SpanningTree,
        iteration: usize,
    ) -> Result<(), WaitingRoomError> {
        log::debug!(
            "[{}] Received NodeAdded message for {}",
            self.node_id,
            node_id
        );
        // We add the node to the member list *before* we check if we need to apply this update.
        // If we get conflicting messages, we'll need to know that this node is a member.
        if !self.network_members.contains(&node_id) {
            self.network_members.push(node_id);
        }

        // The behaviour now is the same as for restructure_tree_message, so we just call that.
        self.restructure_tree_message(tree, iteration)?;
        Ok(())
    }

    pub fn remove_node(&mut self, node_id: NodeId) -> Result<(), WaitingRoomError> {
        log::debug!("[{}] Removing node {}", self.node_id, node_id);

        // If we're the node being removed, something went wrong in the fault detection.
        // We can't continue, so we return an error.
        if node_id == self.node_id {
            return Err(WaitingRoomError::FaultFalsePositive);
        }

        self.network_members.retain(|&x| x != node_id);
        let mut updated_tree = self.spanning_tree.clone();
        updated_tree.remove_node(node_id);

        self.tree_iteration += 1;

        for member in &self.network_members {
            if *member != self.node_id {
                self.network_handle.send_message(
                    *member,
                    NodeToNodeMessage::NodeRemoved(
                        node_id,
                        updated_tree.clone(),
                        self.tree_iteration,
                    ),
                )?;
            }
        }

        self.apply_new_tree(updated_tree)
    }

    pub(super) fn node_remove_message(
        &mut self,
        node_id: NodeId,
        tree: SpanningTree,
        iteration: usize,
    ) -> Result<(), WaitingRoomError> {
        log::debug!(
            "[{}] Received NodeRemoved message from {}",
            self.node_id,
            node_id
        );
        // We remove the node from the member list *before* we check if we need to apply this update.
        // If we get conflicting messages, we'll need to know that this node is not a member.
        self.network_members.retain(|&x| x != node_id);

        // The behaviour now is the same as for restructure_tree_message, so we just call that.
        self.restructure_tree_message(tree, iteration)?;
        Ok(())
    }

    fn restructure_tree(&mut self) -> Result<(), WaitingRoomError> {
        let new_tree = SpanningTree::from_member_list(self.network_members.clone());
        self.tree_iteration += 1;

        for member in &self.network_members {
            if *member != self.node_id {
                self.network_handle.send_message(
                    *member,
                    NodeToNodeMessage::TreeRestructure(new_tree.clone(), self.tree_iteration),
                )?;
            }
        }

        self.apply_new_tree(new_tree)
    }

    pub(super) fn restructure_tree_message(
        &mut self,
        tree: SpanningTree,
        iteration: usize,
    ) -> Result<(), WaitingRoomError> {
        log::debug!("[{}] Received TreeRestructure message (or got triggered by node add or remove message)", self.node_id);
        if iteration == self.tree_iteration {
            // We've either already processed this message, or there is a conflicting change.
            if self.spanning_tree == tree {
                // We've already processed this message.
                log::debug!(
                    "[{}] Ignoring duplicate TreeRestructure message",
                    self.node_id
                );
                return Ok(());
            } else {
                // There is a conflicting change. We need to restructure the tree.
                log::debug!("[{}] Conflicting change detected", self.node_id);
                self.restructure_tree()?;
            }
        }
        if iteration < self.tree_iteration {
            // We've already processed a newer message.
            log::debug!(
                "[{}] Ignoring outdated TreeRestructure message",
                self.node_id
            );
            return Ok(());
        }

        self.tree_iteration = iteration;

        self.apply_new_tree(tree)
    }

    pub fn apply_new_tree(&mut self, tree: SpanningTree) -> Result<(), WaitingRoomError> {
        // We compare the new tree to the old tree to check if we need to add or remove neighbours.
        let old_neighbours = self.spanning_tree.get_node(self.node_id).unwrap().clone();
        let new_neighbours = tree.get_node(self.node_id).unwrap().clone();

        let mut any_added = false;
        let mut any_removed = false;

        // We add all the nodes in the tree to our member list:
        for node in tree.get_node_list() {
            if !self.network_members.contains(&node) {
                self.network_members.push(node);
            }
        }

        for neighbour in old_neighbours.iter() {
            if !new_neighbours.contains(neighbour) {
                // We have a neighbour in the old tree that is not in the new tree.
                // We need to remove this neighbour.
                self.remove_neighbour(*neighbour);
                any_removed = true;
            }
        }

        for neighbour in new_neighbours {
            if !old_neighbours.contains(&neighbour) {
                // We have a neighbour in the new tree that is not in the old tree.
                // We need to add this neighbour.

                // If the neighbour is already in the weight table, some messages got reordered, but that's fine.
                // We still add them, so they get an update if they need it. We don't count them though, since
                // nothing on our side changed.
                self.add_neighbour(neighbour)?;
                if self.qpid_weight_table.get_weight(neighbour).is_none() {
                    any_added = true;
                }
            }
        }

        if any_added && any_removed {
            // We've both added and removed neighbours. After we've received the missing updates, we can re-determine the parent.
            log::debug!(
                "[{}] Both added and removed neighbours. Waiting for updates.",
                self.node_id
            );
            self.qpid_parent = None; // We don't know who the parent should be, so we set it to None.
        } else if any_added {
            // We've only added neighbours. We need to wait for the updates from the new neighbours.
            log::debug!("[{}] Added neighbours. Waiting for updates.", self.node_id);
            self.qpid_parent = None; // We don't know who the parent should be, so we set it to None.
        } else if any_removed {
            // We've only removed neighbours. We can just recompute the parent.
            let new_parent = self.qpid_weight_table.get_smallest().unwrap();
            log::debug!(
                "[{}] Removed neighbours. New parent: {}",
                self.node_id,
                new_parent
            );
            self.qpid_parent = Some(new_parent);
        }
        self.spanning_tree = tree;

        // We'll see if we have all the information we need to set a new QPID parent. If we do, we'll set it.
        if self.qpid_parent.is_none() {
            self.heuristic_set_qpid_parent();
        }

        Ok(())
    }

    fn remove_neighbour(&mut self, neighbour: NodeId) {
        log::debug!("[{}] Removing neighbour {}", self.node_id, neighbour);
        // Removing a neighbour is easier than adding one. We just remove its entry from the table.
        self.qpid_weight_table.remove(neighbour);
    }

    fn add_neighbour(&mut self, neighbour: NodeId) -> Result<(), WaitingRoomError> {
        log::debug!("[{}] Adding neighbour {}", self.node_id, neighbour);
        // Now, we send an update message to our new neighbour.
        let weight = self.qpid_weight_table.compute_weight(neighbour);
        let updated_iteration = self.get_update_iteration(neighbour);
        self.network_handle
            .send_message(neighbour, NodeToNodeMessage::QPIDUpdateMessage {
                weight,
                updated_iteration
            })?;

        Ok(())
    }
}
