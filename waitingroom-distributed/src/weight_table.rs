use waitingroom_core::{time::Time, NodeId};

#[derive(Debug)]
pub struct WeightTable {
    table: Vec<(NodeId, Time)>,
}

impl WeightTable {
    pub fn new() -> Self {
        WeightTable { table: vec![] }
    }

    pub fn from_vec(table: Vec<(NodeId, Time)>) -> Self {
        WeightTable { table }
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
            .filter(|(id, _)| *id != node_id)
            .fold(Time::MAX, |min_weight, (_, weight)| min_weight.min(*weight))
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
}
