use waitingroom_core::{network::DummyNetwork, settings::GeneralWaitingRoomSettings};
use waitingroom_distributed::messages::NodeToNodeMessage;

use crate::Node;

#[derive(Debug)]
pub enum InvariantCheckError {
    QpidNode,
    SingleRoot,
    TooManyOnSite,
}

pub fn check_consistent_state(
    nodes: &[Node],
    network: &DummyNetwork<NodeToNodeMessage>,
    settings: &GeneralWaitingRoomSettings,
) -> Result<(), InvariantCheckError> {
    if network.is_empty() {
        // The QPID invariant only makes sense to check if we have no network messages.
        // Otherwise, we might be in the middle of a QPID operation, in which case the
        // invariant doesn't have to hold.
        if !verify_qpid_invariant(nodes) {
            return Err(InvariantCheckError::QpidNode);
        }
    }

    if !ensure_only_single_root(nodes) {
        return Err(InvariantCheckError::SingleRoot);
    }

    if !ensure_no_more_than_n_onsite(nodes, settings.max_user_count) {
        return Err(InvariantCheckError::TooManyOnSite);
    }

    log::debug!("All invariants hold");
    Ok(())
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

        let w_v = v
            .get_qpid_weight_table()
            .get_weight(v.get_node_id())
            .unwrap();

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
                "Invariant failed for node {}. Min weight is {:?}, w_v_parent_v is {:?}",
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
    if root_count > 1 {
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

fn ensure_no_more_than_n_onsite(nodes: &[Node], max_users: usize) -> bool {
    let mut onsite_count = 0;
    for node in nodes {
        onsite_count += node.get_local_on_site_count();
    }
    if onsite_count > max_users {
        log::error!(
            "There should be at most {} users on site. Found {} users on site.",
            max_users,
            onsite_count
        );
        return false;
    }
    true
}

// TODO move to simulation
// pub fn check_final_state(_nodes: &[Node], users: &[User]) -> Result<(), FinalStateCheckError> {
//     log::info!("Validating results");

//     // We verify that the users are let out in the correct order.
//     // dbg!(&users);
//     let mut prev_eviction_time = 0;
//     for (i, user) in users.iter().enumerate() {
//         let eviction_time = match user.get_eviction_time() {
//             Some(t) => t,
//             None => u128::MAX,
//         };
//         if eviction_time < prev_eviction_time {
//             return Err(FinalStateCheckError::UsersWrongOrder(
//                 i,
//                 prev_eviction_time,
//                 eviction_time,
//             ));
//         }
//         prev_eviction_time = eviction_time;
//     }

//     let total_users_processed = users
//         .iter()
//         .filter(|u| u.get_eviction_time().is_some())
//         .count();
//     let total_users = users.len();
//     log::info!(
//         "Processed {} out of {} users",
//         total_users_processed,
//         total_users
//     );
//     Ok(())
// }
