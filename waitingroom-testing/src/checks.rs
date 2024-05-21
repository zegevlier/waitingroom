use crate::Node;

pub fn assert_consistent_state(nodes: &[Node]) {
    verify_qpid_invariant(nodes);
    ensure_only_single_root(nodes);
}

fn verify_qpid_invariant(nodes: &[Node]) {
    for v in nodes.iter() {
        let parent_v = v.get_qpid_parent().unwrap();

        let w_v_parent_v = v.get_qpid_weight_table().compute_weight(parent_v);

        let w_v = v.get_qpid_weight_table().get(v.get_node_id()).unwrap();

        let mut min_weight = w_v;

        // Now we look at all nodes, check if their parent is the current node, and if so, check if their weight is less than the parent weight
        for x in nodes.iter() {
            if x.get_qpid_parent().unwrap() == v.get_node_id() {
                let w_x_v = x.get_qpid_weight_table().compute_weight(v.get_node_id());
                min_weight = min_weight.min(w_x_v);
            }
        }

        // Now we assert the invariant
        assert_eq!(
            min_weight,
            w_v_parent_v,
            "Invariant failed for node {}",
            v.get_node_id()
        );
    }
}

fn ensure_only_single_root(nodes: &[Node]) {
    let mut root_count = 0;
    for node in nodes {
        if node.get_qpid_parent() == Some(node.get_node_id()) {
            root_count += 1;
        }
    }
    assert_eq!(root_count, 1, "There should be exactly one root node");
}
