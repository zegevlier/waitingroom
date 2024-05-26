use std::vec;

use waitingroom_core::NodeId;

type AdjacencyList = Vec<(NodeId, Vec<usize>)>;
type Edge = (NodeId, NodeId);

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SpanningTree {
    adjacency_list: AdjacencyList,
}

impl SpanningTree {
    pub fn new_empty() -> Self {
        SpanningTree {
            adjacency_list: Vec::new(),
        }
    }

    /// Create a new spanning tree from a list of members.
    /// This function is designed to always return the same tree for the same members,
    /// regardless of the order of the members.
    pub fn from_member_list(members: Vec<NodeId>) -> Self {
        let mut members = members;
        members.sort();
        members.dedup();

        let adjacency_list = members
            .iter()
            .map(|id| (*id, Vec::new()))
            .collect::<Vec<(NodeId, Vec<NodeId>)>>();

        let mut spanning_tree = SpanningTree { adjacency_list };
        spanning_tree.reconnect();
        spanning_tree
    }

    pub fn add_node(&mut self, node_id: NodeId) -> Vec<Edge> {
        // Add a new node to the graph.
        self.adjacency_list.push((node_id, Vec::new()));

        // Reconnect the graph until there is only one connected component.
        self.reconnect()
    }

    pub fn remove_node(&mut self, node_id: NodeId) -> Vec<Edge> {
        // Remove the node with the given ID from the graph.
        self.adjacency_list.retain(|(id, _)| *id != node_id);
        self.adjacency_list
            .iter_mut()
            .for_each(|(_, neighbors)| neighbors.retain(|n| *n != node_id));

        // Reconnect the graph until there is only one connected component.
        self.reconnect()
    }

    pub fn get_node_list(&self) -> Vec<NodeId> {
        self.adjacency_list.iter().map(|(id, _)| *id).collect()
    }

    /// This function finds the neighbour of the given node that is towards the node with the lowest ID in the tree.
    pub fn towards_lowest_id(&self, node_id: NodeId) -> NodeId {
        let lowest_id = self.adjacency_list.iter().map(|(id, _)| id).min().unwrap();
        if node_id == *lowest_id {
            return node_id;
        }
        // Since we're in a spanning tree, there is only one path to the "root" node.
        // We just need to find the neighbor of `node_id` that is on the path to the node with the lowest ID.
        // We don't know the order of the neighbours' IDs.
        // If we start at node 3, and the lowest node is node 0, and to get from 3 to 0, we need to go 3 -> 4 -> 1 -> 0, we return 4.

        let mut visited = vec![node_id];
        let mut stack = vec![node_id];
        let mut parents = Vec::new();

        while let Some(current_node) = stack.pop() {
            for neighbor in self.get_node(current_node).unwrap() {
                if !visited.contains(neighbor) {
                    stack.push(*neighbor);
                    parents.push((current_node, *neighbor));
                    visited.push(*neighbor);
                    // We've found the path to the lowest node, so we can stop.
                    if neighbor == lowest_id {
                        stack.clear();
                        break;
                    }
                }
            }
        }

        // Now we walk back from the lowest node to the node with the given ID.
        let mut current_node = *lowest_id;
        while current_node != node_id {
            let parent = parents
                .iter()
                .find(|(_, child)| child == &current_node)
                .unwrap()
                .0;
            current_node = parent;
        }

        parents
            .iter()
            .find(|(parent, _)| parent == &current_node)
            .unwrap()
            .1
    }

    /// Reconnect all nodes in the graph until there is only one connected component.
    /// Returns a vector of all the edges that were added.
    fn reconnect(&mut self) -> Vec<Edge> {
        let mut added_edges = Vec::new();

        loop {
            let components = self.find_connected_components();

            if components.len() == 1 || components.is_empty() {
                break;
            }

            let component_1 = &components[0];
            let component_2 = &components[1];

            let first_node = self.find_best_node(component_1);
            let second_node = self.find_best_node(component_2);

            let new_edge = self.add_edge(first_node, second_node);

            added_edges.push(new_edge);
        }
        added_edges
    }

    /// Find all connected components in the graph.
    /// Returns a vector of vectors, where each vector contains all the node IDs of nodes in the connected component.
    fn find_connected_components(&mut self) -> Vec<Vec<NodeId>> {
        let mut visited = Vec::new();
        let mut components = Vec::new();

        for (node_id, _) in &self.adjacency_list {
            if !visited.contains(node_id) {
                let mut stack = vec![*node_id];
                let mut component = Vec::new();

                while let Some(current_node) = stack.pop() {
                    visited.push(current_node);
                    component.push(current_node);

                    for neighbor in self.get_node(current_node).unwrap().iter() {
                        if !visited.contains(neighbor) {
                            stack.push(*neighbor);
                        }
                    }
                }
                components.push(component);
            }
        }
        components
    }

    /// Get a mutable reference to the element in the adjacency list with the given node ID.
    fn get_node_mut(&mut self, node_id: NodeId) -> Option<&mut Vec<NodeId>> {
        self.adjacency_list
            .iter_mut()
            .find(|(id, _)| *id == node_id)
            .map(|(_, neighbors)| neighbors)
    }

    /// Get a reference to the element in the adjacency list with the given node ID.
    pub fn get_node(&self, node_id: NodeId) -> Option<&Vec<NodeId>> {
        self.adjacency_list
            .iter()
            .find(|(id, _)| *id == node_id)
            .map(|(_, neighbors)| neighbors)
    }

    /// Add an edge between two nodes in the graph. If the nodes are already connected, nothing happens.
    fn add_edge(&mut self, first_node: NodeId, second_node: NodeId) -> Edge {
        if self
            .get_node_mut(first_node)
            .unwrap()
            .contains(&second_node)
        {
            return (first_node, second_node);
        }

        self.get_node_mut(first_node).unwrap().push(second_node);
        self.get_node_mut(second_node).unwrap().push(first_node);

        (first_node, second_node)
    }

    /// This functions tries to find the optimal element from a component to connect to another component.
    /// We want a component is both close to other nodes, as to not increase the length of the maximum path,
    /// and doesn't have too many neighbors, as to make sure that the impact of any single node going down is limited.
    /// The function returns the node ID of the best node to connect to another component.
    fn find_best_node(&self, component: &Vec<NodeId>) -> NodeId {
        // We have some constants to decide how each part of the score is weighted.
        // The higher the weight, the more important that part is.
        // The "correct" values for these have to be determined experimentally,
        // these were chosen arbitrarily.
        let neigbour_weight = 2;
        let depth_weight = 5;

        let mut best_node = 0;
        // Each node gets a score, the lower the better.
        let mut best_score = usize::MAX;

        for node in component {
            let mut score = 0;

            // The first part of the score is the number of neighbors. We want to minimize this.
            let neighbors = self.get_node(*node).unwrap();
            score += neighbors.len() * neigbour_weight;

            // The second part of the score is the length of the longest path to another node.
            // We want to minimize this as well.

            // We use a stack to do a depth-first search.
            let mut stack = vec![(*node, 0)];
            let mut visited = Vec::new();
            let mut max_depth = 0;

            while let Some((current_node, depth)) = stack.pop() {
                visited.push(current_node);
                max_depth = max_depth.max(depth);

                for neighbor in self.get_node(current_node).unwrap() {
                    if !visited.contains(neighbor) {
                        stack.push((*neighbor, depth + 1));
                    }
                }
            }

            score += max_depth * depth_weight;

            if score < best_score {
                best_score = score;
                best_node = *node;
            }
        }

        best_node
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tree() {
        let spanning_tree = SpanningTree::new_empty();
        assert!(spanning_tree.adjacency_list.is_empty());
        assert!(ensure_all_rechable(&spanning_tree));
    }

    #[test]
    fn one_element_tree() {
        let mut spanning_tree = SpanningTree::new_empty();
        let edges = spanning_tree.add_node(0);
        assert_eq!(edges.len(), 0);
        assert_eq!(spanning_tree.adjacency_list.len(), 1);
        assert!(ensure_all_rechable(&spanning_tree));
    }

    #[test]
    fn two_element_tree() {
        let mut spanning_tree = SpanningTree::new_empty();
        let edges = spanning_tree.add_node(0);
        assert_eq!(edges.len(), 0);
        let edges = spanning_tree.add_node(1);
        assert_eq!(edges.len(), 1);
        assert_eq!(spanning_tree.adjacency_list.len(), 2);
        assert!(ensure_all_rechable(&spanning_tree));
    }

    #[test]
    fn ten_element_tree() {
        let mut spanning_tree = SpanningTree::new_empty();
        for i in 0..10 {
            let _ = spanning_tree.add_node(i);
        }
        assert_eq!(spanning_tree.adjacency_list.len(), 10);
        assert!(ensure_all_rechable(&spanning_tree));
    }

    #[test]
    fn remove_node() {
        let mut spanning_tree = SpanningTree::new_empty();
        for i in 0..10 {
            let _ = spanning_tree.add_node(i);
        }
        let edges = spanning_tree.remove_node(0);
        dbg!(edges);
        dbg!(&spanning_tree.adjacency_list);
        assert_eq!(spanning_tree.adjacency_list.len(), 9);
        assert!(ensure_all_rechable(&spanning_tree));
    }

    #[test]
    fn remove_node_2() {
        let mut spanning_tree = SpanningTree::new_empty();
        for i in 0..10 {
            let _ = spanning_tree.add_node(i);
        }
        let edges = spanning_tree.remove_node(1);
        dbg!(edges);
        dbg!(&spanning_tree.adjacency_list);
        assert_eq!(spanning_tree.adjacency_list.len(), 9);
        assert!(ensure_all_rechable(&spanning_tree));
    }

    #[test]
    fn remove_node_3() {
        let mut spanning_tree = SpanningTree::new_empty();
        for i in 0..10 {
            let _ = spanning_tree.add_node(i);
        }
        let edges = spanning_tree.remove_node(9);
        dbg!(edges);
        dbg!(&spanning_tree.adjacency_list);
        assert_eq!(spanning_tree.adjacency_list.len(), 9);
        assert!(ensure_all_rechable(&spanning_tree));
    }

    #[test]
    fn remove_node_multiple() {
        let mut spanning_tree = SpanningTree::new_empty();
        for i in 0..10 {
            let _ = spanning_tree.add_node(i);
        }
        let edges = spanning_tree.remove_node(5);
        dbg!(edges);
        dbg!(&spanning_tree.adjacency_list);
        assert_eq!(spanning_tree.adjacency_list.len(), 9);
        assert!(ensure_all_rechable(&spanning_tree));

        let edges = spanning_tree.remove_node(6);
        dbg!(edges);
        dbg!(&spanning_tree.adjacency_list);
        assert_eq!(spanning_tree.adjacency_list.len(), 8);
        assert!(ensure_all_rechable(&spanning_tree));

        let _ = spanning_tree.remove_node(0);
        assert_eq!(spanning_tree.adjacency_list.len(), 7);
        assert!(ensure_all_rechable(&spanning_tree));
    }

    #[test]
    fn create_from_member_list() {
        let members = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let spanning_tree = SpanningTree::from_member_list(members);
        assert_eq!(spanning_tree.adjacency_list.len(), 10);
        assert!(ensure_all_rechable(&spanning_tree));
        let members2 = vec![9, 8, 7, 6, 5, 4, 3, 2, 1, 0];
        let spanning_tree2 = SpanningTree::from_member_list(members2);
        assert_eq!(spanning_tree.adjacency_list, spanning_tree2.adjacency_list);

        let members3 = vec![3, 2, 5, 4, 1, 0, 7, 6, 9, 8];
        let spanning_tree3 = SpanningTree::from_member_list(members3);
        assert_eq!(spanning_tree.adjacency_list, spanning_tree3.adjacency_list);
    }

    fn ensure_all_rechable(spanning_tree: &SpanningTree) -> bool {
        for (node_id, _) in &spanning_tree.adjacency_list {
            let mut visited = Vec::new();
            let mut stack = vec![*node_id];
            while let Some(current_node) = stack.pop() {
                visited.push(current_node);
                for neighbor in spanning_tree.get_node(current_node).unwrap() {
                    if !visited.contains(neighbor) {
                        stack.push(*neighbor);
                    }
                }
            }
            if visited.len() != spanning_tree.adjacency_list.len() {
                return false;
            }
        }
        true
    }
}
