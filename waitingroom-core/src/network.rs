use std::{cell::RefCell, fmt::Debug, rc::Rc};

use log;

use crate::{error::NetworkError, NodeId};

#[derive(Debug)]
pub struct Message<M> {
    pub from_node: NodeId,
    pub to_node: NodeId,
    pub message: M,
}

pub trait Network<M>: std::fmt::Debug {
    type NetworkHandle: NetworkHandle<M>;

    fn join(&self, node: NodeId) -> Result<Self::NetworkHandle, NetworkError>;

    fn all_nodes(&self) -> Result<Vec<NodeId>, NetworkError>;
}

pub trait NetworkHandle<M>: Debug {
    fn send_message(&self, to_node: NodeId, message: M) -> Result<(), NetworkError>;
    fn receive_message(&self) -> Result<Option<Message<M>>, NetworkError>;
}

#[derive(Clone, Debug)]
pub struct DummyNetwork<M>
where
    M: Clone,
{
    // Using `RefCell` here is not ideal, but it works for this use-case.
    // It seems like the best option for now, and since this is only the mock it doesn't *really* matter.
    nodes: Rc<RefCell<Vec<NodeId>>>,
    messages: Rc<RefCell<Vec<Message<M>>>>,
}

impl<M> Network<M> for DummyNetwork<M>
where
    M: Debug + Clone,
{
    type NetworkHandle = DummyNetworkHandle<M>;

    fn join(&self, node: NodeId) -> Result<Self::NetworkHandle, NetworkError> {
        log::debug!("[NET] Node {} joined", node);
        if !self.add_node(node) {
            return Err(NetworkError::NodeIDAlreadyUsed);
        }
        Ok(DummyNetworkHandle {
            node,
            network: self.clone(),
        })
    }

    fn all_nodes(&self) -> Result<Vec<NodeId>, NetworkError> {
        log::debug!("[NET] Getting all nodes");
        Ok(self.nodes.borrow().clone())
    }
}

impl<M> DummyNetwork<M>
where
    M: Clone,
{
    pub fn new() -> Self {
        Self {
            nodes: Rc::new(RefCell::new(Vec::new())),
            messages: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn add_node(&self, node: NodeId) -> bool {
        if self.nodes.borrow().contains(&node) {
            false
        } else {
            self.nodes.borrow_mut().push(node);
            true
        }
    }

    fn send_message(
        &self,
        from_node: NodeId,
        to_node: NodeId,
        message: M,
    ) -> Result<(), NetworkError> {
        if !self.nodes.borrow().contains(&to_node) {
            return Err(NetworkError::DestNodeNotFound);
        }
        self.messages.borrow_mut().push(Message {
            from_node,
            to_node,
            message,
        });
        Ok(())
    }

    fn receive_message(&self, node: NodeId) -> Result<Option<Message<M>>, NetworkError> {
        let mut messages = self.messages.borrow_mut();
        let index = messages.iter().position(|m| m.to_node == node);
        if let Some(index) = index {
            Ok(Some(messages.remove(index)))
        } else {
            Ok(None)
        }
    }
}

impl<M> Default for DummyNetwork<M>
where
    M: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

pub struct DummyNetworkHandle<M>
where
    M: Clone,
{
    node: NodeId,
    network: DummyNetwork<M>,
}

impl<M: Debug> Debug for DummyNetworkHandle<M>
where
    M: Clone,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DummyNetworkHandle")
            .field("node", &self.node)
            .field("network", &"...")
            .finish()
    }
}

impl<M> NetworkHandle<M> for DummyNetworkHandle<M>
where
    M: Debug + Clone,
{
    fn send_message(&self, to_node: NodeId, message: M) -> Result<(), NetworkError> {
        log::debug!("[NET] {} -> {}: {:?}", self.node, to_node, message);
        self.network.send_message(self.node, to_node, message)
    }

    fn receive_message(&self) -> Result<Option<Message<M>>, NetworkError> {
        self.network.receive_message(self.node)
    }
}
