use waitingroom_core::{time::Time, NodeId};

#[derive(Debug)]
pub struct WeightTable {
    table: Vec<(NodeId, Time)>,
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
        WeightTable { table, node_id }
    }

    pub fn get(&self, node_id: NodeId) -> Option<Time> {
        self.table
            .iter()
            .find(|(id, _)| *id == node_id)
            .map(|(_, time)| *time)
    }

    pub fn set(&mut self, node_id: NodeId, time: Time) {
        if let Some((_, t)) = self.table.iter_mut().find(|(id, _)| *id == node_id) {
            *t = time;
        } else {
            self.table.push((node_id, time));
        }
    }

    pub fn compute_weight(&self, node_id: NodeId) -> Time {
        self.table
            .iter()
            .filter(|(id, _)| node_id == self.node_id || *id != node_id)
            .map(|(_, time)| *time)
            .min()
            .unwrap_or(Time::MAX)
    }

    pub fn get_smallest(&self) -> Option<NodeId> {
        self.table
            .iter()
            .min_by_key(|(_, time)| *time)
            .map(|(id, _)| *id)
    }

    pub fn any_not_max(&self) -> bool {
        self.table.iter().any(|(_, time)| *time != Time::MAX)
    }

    pub fn neighbour_count(&self) -> usize {
        self.table.len() - 1 // We don't count ourselves, but we are in the table
    }

    pub fn all_neighbours(&self) -> Vec<NodeId> {
        self.table.iter().map(|(id, _)| *id).collect()
    }

    pub fn all_weights(&self) -> Vec<(NodeId, Time)> {
        self.table.clone()
    }
}
