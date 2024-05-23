use std::{cell::{RefCell, RefMut}, fmt::Debug, rc::Rc};

use log;

use crate::{
    error::NetworkError,
    random::{DeterministicRandomProvider, RandomProvider},
    time::{DummyTimeProvider, TimeProvider},
    NodeId,
};

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

#[derive(Clone)]
pub enum Latency {
    Fixed(u128),
    Random(u128, u128, DeterministicRandomProvider),
}

impl Debug for Latency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fixed(arg0) => f.debug_tuple("Fixed").field(arg0).finish(),
            Self::Random(arg0, arg1, _) => f.debug_tuple("Random").field(arg0).field(arg1).finish(),
        }
    }
}

#[derive(Debug)]
pub struct DummyMessage<M> {
    message: Message<M>,
    arrival_time: u128,
}

#[derive(Clone, Debug)]
pub struct DummyNetwork<M>
where
    M: Clone + Debug,
{
    // Using `RefCell` here is not ideal, but it works for this use-case.
    // It seems like the best option for now, and since this is only the mock it doesn't *really* matter.
    nodes: Rc<RefCell<Vec<NodeId>>>,
    messages: Rc<RefCell<Vec<DummyMessage<M>>>>,
    time_provider: DummyTimeProvider,
    latency: Latency,
}

impl<M> Network<M> for DummyNetwork<M>
where
    M: Clone + Debug,
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
    M: Clone + Debug,
{
    pub fn new(time_provider: DummyTimeProvider, latency: Latency) -> Self {
        Self {
            nodes: Rc::new(RefCell::new(Vec::new())),
            messages: Rc::new(RefCell::new(Vec::new())),
            time_provider,
            latency,
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
        let latency = match &self.latency {
            Latency::Fixed(latency) => *latency,
            Latency::Random(min, max, random_provider) => {
                let random = random_provider.random_u64() as u128;
                min + random % (max - min)
            }
        };

        let now_time = self.time_provider.get_now_time();

        self.messages.borrow_mut().push(DummyMessage {
            message: Message {
                from_node,
                to_node,
                message,
            },
            arrival_time: now_time + latency,
        });
        Ok(())
    }

    fn receive_message(&self, node: NodeId) -> Result<Option<Message<M>>, NetworkError> {
        let mut messages = self.messages.borrow_mut();
        let now_time = self.time_provider.get_now_time();
        let index = messages
            .iter()
            .position(|m| m.message.to_node == node && m.arrival_time <= now_time);
        if let Some(index) = index {
            let message = messages.remove(index).message;
            log::debug!(
                "[NET] {} <- {}: {:?}",
                node,
                message.from_node,
                message.message
            );
            Ok(Some(message))
        } else {
            Ok(None)
        }
    }

    pub fn len(&self) -> usize {
        self.messages.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.borrow().is_empty()
    }

    pub fn get_messages_mut(&self) -> RefMut<Vec<DummyMessage<M>>> {
        self.messages.borrow_mut()
    }
}

pub struct DummyNetworkHandle<M>
where
    M: Clone + Debug,
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
    M: Clone + Debug,
{
    fn send_message(&self, to_node: NodeId, message: M) -> Result<(), NetworkError> {
        log::debug!("[NET] {} -> {}: {:?}", self.node, to_node, message);
        self.network.send_message(self.node, to_node, message)
    }

    fn receive_message(&self) -> Result<Option<Message<M>>, NetworkError> {
        self.network.receive_message(self.node)
    }
}
