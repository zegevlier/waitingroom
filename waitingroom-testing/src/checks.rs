use crate::{debug_print_qpid_info_for_nodes, Node};

pub fn assert_consistent_state(nodes: &[Node]) {
    if !verify_qpid_invariant(nodes) {
        debug_print_qpid_info_for_nodes(nodes);
        panic!("QPID invariant check failed");
    } else {
        log::debug!("QPID invariant holds");
    }
    if !ensure_only_single_root(nodes) {
        debug_print_qpid_info_for_nodes(nodes);
        panic!("Multiple roots found");
    } else {
        log::debug!("Single root invariant holds");
    }
    log::info!("All invariants hold");
}

fn verify_qpid_invariant(nodes: &[Node]) -> bool {
    for v in nodes.iter() {
        let parent_v = match v.get_qpid_parent() {
            Some(p) => p,
            None => {
                log::error!("Node {} has no parent", v.get_node_id());
                return false;
            }
        };

        let w_v_parent_v = v.get_qpid_weight_table().compute_weight(parent_v);

        let w_v = v.get_qpid_weight_table().get(v.get_node_id()).unwrap();

        let mut min_weight = w_v;

        // Now we look at all nodes, check if their parent is the current node, and if so, check if their weight is less than the parent weight
        for x in nodes.iter() {
            let x_parent = match x.get_qpid_parent() {
                Some(p) => p,
                None => {
                    log::error!("Node {} has no parent", x.get_node_id());
                    return false;
                }
            };
            if x_parent == v.get_node_id() {
                let w_x_v = x.get_qpid_weight_table().compute_weight(v.get_node_id());
                min_weight = min_weight.min(w_x_v);
            }
        }

        // Now we assert the invariant
        if min_weight != w_v_parent_v {
            log::error!(
                "Invariant failed for node {}. Min weight is {}, w_v_parent_v is {}",
                v.get_node_id(),
                min_weight,
                w_v_parent_v
            );
            return false;
        }
    }

    true
}

fn ensure_only_single_root(nodes: &[Node]) -> bool {
    let mut root_count = 0;
    for node in nodes {
        if node.get_qpid_parent() == Some(node.get_node_id()) {
            root_count += 1;
        }
    }
    if root_count != 1 {
        log::error!(
            "There should be exactly one root node. Found {} root nodes.",
            root_count
        );
        for node in nodes {
            if node.get_qpid_parent() == Some(node.get_node_id()) {
                log::error!("Root node: {}", node.get_node_id());
            }
        }
        return false;
    }
    true
}
