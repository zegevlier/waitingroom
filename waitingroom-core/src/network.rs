use std::{cell::RefCell, fmt::Debug, rc::Rc};

use log;

use crate::NodeId;

#[derive(Debug)]
pub enum NetworkJoinError {
    NodeAlreadyPresent,
}

#[derive(Debug)]
pub enum NetworkError {
    ConnectionError,
}

#[derive(Debug)]
pub enum MessageSendError {
    NodeNotFound,
}

#[derive(Debug)]
pub enum ReceiveError {
    ConnectionError,
}

pub struct Message<M> {
    pub from_node: NodeId,
    pub to_node: NodeId,
    pub message: M,
}

pub trait Network<M> {
    type NetworkHandle: NetworkHandle<M>;

    fn join(&self, node: NodeId) -> Result<Self::NetworkHandle, NetworkJoinError>;

    fn all_nodes(&self) -> Result<Vec<NodeId>, NetworkError>;
}

pub trait NetworkHandle<M> {
    fn send_message(&self, to_node: NodeId, message: M) -> Result<(), MessageSendError>;
    fn receive_message(&self) -> Result<Option<Message<M>>, ReceiveError>;
}

#[derive(Clone)]
pub struct DummyNetwork<M>
where
    M: Clone,
{
    nodes: Rc<RefCell<Vec<NodeId>>>,
    messages: Rc<RefCell<Vec<Message<M>>>>,
}

impl<M> Network<M> for DummyNetwork<M>
where
    M: Debug + Clone,
{
    type NetworkHandle = DummyNetworkHandle<M>;

    fn join(&self, node: NodeId) -> Result<Self::NetworkHandle, NetworkJoinError> {
        log::debug!("[NET] Node {} joined", node);
        if !self.add_node(node) {
            return Err(NetworkJoinError::NodeAlreadyPresent);
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
    ) -> Result<(), MessageSendError> {
        if !self.nodes.borrow().contains(&to_node) {
            return Err(MessageSendError::NodeNotFound);
        }
        self.messages.borrow_mut().push(Message {
            from_node,
            to_node,
            message,
        });
        Ok(())
    }

    fn receive_message(&self, node: NodeId) -> Result<Option<Message<M>>, ReceiveError> {
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

impl<M> NetworkHandle<M> for DummyNetworkHandle<M>
where
    M: Debug + Clone,
{
    fn send_message(&self, to_node: NodeId, message: M) -> Result<(), MessageSendError> {
        log::debug!("[NET] {} -> {}: {:?}", self.node, to_node, message);
        self.network.send_message(self.node, to_node, message)
    }

    fn receive_message(&self) -> Result<Option<Message<M>>, ReceiveError> {
        log::debug!("[NET] {} checking for messages", self.node);
        self.network.receive_message(self.node)
    }
}
