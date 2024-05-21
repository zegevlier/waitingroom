// This module makes available methods for testing the distributed waiting room.
// None of these methods are intended for production use.

use waitingroom_core::{network::Network, random::RandomProvider, time::TimeProvider, NodeId};

use crate::{messages::NodeToNodeMessage, weight_table::WeightTable, DistributedWaitingRoom};

impl<T, R, N> DistributedWaitingRoom<T, R, N>
where
    T: TimeProvider,
    R: RandomProvider,
    N: Network<NodeToNodeMessage>,
{
    pub fn get_qpid_weight_table(&self) -> &WeightTable {
        &self.qpid_weight_table
    }

    pub fn get_qpid_parent(&self) -> Option<NodeId> {
        self.qpid_parent
    }

    pub fn get_node_id(&self) -> NodeId {
        self.node_id
    }
}
