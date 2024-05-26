use waitingroom_core::{time::Time, NodeId};

#[derive(Debug, Clone)]
struct Entry {
    update_iteration: u64,
    weight: Time,
}

#[derive(Debug)]
pub struct WeightTable {
    table: Vec<(NodeId, Entry)>,
    /// The ID of the node that this weight table belongs to
    node_id: NodeId,
}

impl WeightTable {
    pub fn new(node_id: NodeId) -> Self {
        WeightTable {
            table: vec![],
            node_id,
        }
    }

    pub fn from_vec(node_id: NodeId, table: Vec<(NodeId, Time)>) -> Self {
        WeightTable {
            table: table
                .iter()
                .map(|(i, t)| {
                    (
                        *i,
                        Entry {
                            weight: *t,
                            update_iteration: 0,
                        },
                    )
                })
                .collect(),
            node_id,
        }
    }

    pub fn get_weight(&self, node_id: NodeId) -> Option<Time> {
        self.table
            .iter()
            .find(|(id, _)| *id == node_id)
            .map(|(_, entry)| entry.weight)
    }

    pub fn get_last_update(&self, node_id: NodeId) -> Option<u64> {
        self.table
            .iter()
            .find(|(id, _)| *id == node_id)
            .map(|(_, entry)| entry.update_iteration)
    }

    pub fn set(&mut self, node_id: NodeId, weight: Time, update_iteration: u64) {
        if let Some(prev_last_update) = self.get_last_update(node_id) {
            if update_iteration < prev_last_update {
                log::warn!(
                    "[NODE {}] Tried to set weight for node {} with last_update {} to {} but it was already set to {}",
                    self.node_id,
                    node_id,
                    update_iteration,
                    weight,
                    prev_last_update
                );
                return;
            }
        }
        if let Some((_, t)) = self.table.iter_mut().find(|(id, _)| *id == node_id) {
            *t = Entry {
                update_iteration,
                weight,
            };
        } else {
            self.table.push((
                node_id,
                Entry {
                    update_iteration,
                    weight,
                },
            ));
        }
    }

    pub fn remove(&mut self, node_id: NodeId) {
        self.table.retain(|(id, _)| *id != node_id);
    }

    pub fn compute_weight(&self, node_id: NodeId) -> Time {
        self.table
            .iter()
            .filter(|(id, _)| node_id == self.node_id || *id != node_id)
            .map(|(_, entry)| entry.weight)
            .min()
            .unwrap_or(Time::MAX)
    }

    pub fn get_smallest(&self) -> Option<NodeId> {
        self.table
            .iter()
            .map(|(id, entry)| (*id, entry.weight))
            .min_by_key(|(_, time)| *time)
            .map(|(id, _)| id)
    }

    pub fn any_not_max(&self) -> bool {
        self.table
            .iter()
            .any(|(_, entry)| entry.weight != Time::MAX)
    }

    pub fn neighbour_count(&self) -> usize {
        self.table.len() - 1 // We don't count ourselves, but we are in the table
    }

    pub fn all_neighbours(&self) -> Vec<NodeId> {
        self.table.iter().map(|(id, _)| *id).collect()
    }

    pub fn all_weights(&self) -> Vec<(NodeId, Time)> {
        self.table
            .iter()
            .map(|(id, entry)| (*id, entry.weight))
            .collect()
    }
}
