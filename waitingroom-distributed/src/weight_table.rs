use waitingroom_core::{ticket::TicketIdentifier, time::Time, NodeId};

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Weight {
    join_time: Time,
    ticket_id: TicketIdentifier,
    node_id: NodeId,
}

impl Weight {
    pub fn new(join_time: Time, ticket_id: TicketIdentifier, node_id: NodeId) -> Self {
        Weight {
            join_time,
            ticket_id,
            node_id,
        }
    }

    pub fn is_max(&self) -> bool {
        self.join_time == Time::MAX
    }
}

impl PartialEq for Weight {
    fn eq(&self, other: &Self) -> bool {
        self.join_time == other.join_time && self.ticket_id == other.ticket_id
    }
}

impl Eq for Weight {}

impl PartialOrd for Weight {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Weight {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // We first compare on join time, then on ticket id, and finally on node id
        self.join_time
            .cmp(&other.join_time)
            .then_with(|| self.ticket_id.cmp(&other.ticket_id))
            .then_with(|| self.node_id.cmp(&other.node_id))
    }
}

#[derive(Debug, Clone)]
struct Entry {
    update_iteration: u64,
    weight: Weight,
}

#[derive(Debug)]
pub struct WeightTable {
    table: Vec<(NodeId, Entry)>,
    /// The ID of the node that this weight table belongs to
    node_id: NodeId,
    true_neighbours: Vec<NodeId>,
}

impl WeightTable {
    pub fn new(node_id: NodeId) -> Self {
        WeightTable {
            table: vec![],
            node_id,
            true_neighbours: vec![node_id],
        }
    }

    pub fn from_vec(node_id: NodeId, table: Vec<(NodeId, Weight)>) -> Self {
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
            true_neighbours: table.iter().map(|(i, _)| *i).collect(),
        }
    }

    pub fn get_weight(&self, node_id: NodeId) -> Option<Weight> {
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

    pub fn set(&mut self, node_id: NodeId, weight: Weight, update_iteration: u64) {
        if let Some(prev_last_update) = self.get_last_update(node_id) {
            if update_iteration < prev_last_update {
                log::info!(
                    "[NODE {}] Tried to set weight for node {} with last_update {} to {:?} but it was already set to {}",
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

    pub fn compute_weight(&self, node_id: NodeId) -> Weight {
        self.compute_weight_allowlist(node_id, &self.true_neighbours)
    }

    pub fn compute_weight_allowlist(&self, node_id: NodeId, allowing: &[NodeId]) -> Weight {
        self.table
            .iter()
            .filter(|(id, _)| node_id == self.node_id || *id != node_id)
            .filter(|(id, _)| allowing.contains(id))
            .map(|(_, entry)| entry.weight)
            .min()
            .unwrap_or(Weight::new(Time::MAX, 0, self.node_id))
    }

    pub fn get_smallest(&self) -> Option<NodeId> {
        self.table
            .iter()
            .filter(|(id, _)| self.true_neighbours.contains(id))
            .map(|(id, entry)| (*id, entry.weight))
            .min_by_key(|(_, time)| *time)
            .map(|(id, _)| id)
    }

    pub fn any_not_max(&self) -> bool {
        self.table
            .iter()
            .filter(|(id, _)| self.true_neighbours.contains(id))
            .any(|(_, entry)| !entry.weight.is_max())
    }

    pub fn neighbour_count(&self) -> usize {
        self.table
            .iter()
            .filter(|(n, _)| self.true_neighbours.contains(n))
            .count()
            - 1 // We don't count ourselves, but we are in the table
    }

    pub fn get_all_neighbours(&self) -> Vec<NodeId> {
        self.table.iter().map(|(id, _)| *id).collect()
    }

    pub fn all_weights(&self) -> Vec<(NodeId, Weight)> {
        self.table
            .iter()
            .map(|(id, entry)| (*id, entry.weight))
            .collect()
    }

    pub fn set_true_neighbours(&mut self, true_neighbours: Vec<NodeId>) {
        self.true_neighbours = true_neighbours;
        self.true_neighbours.push(self.node_id);
    }

    pub fn get_true_neighbours(&self) -> Vec<NodeId> {
        self.true_neighbours.clone()
    }
}
