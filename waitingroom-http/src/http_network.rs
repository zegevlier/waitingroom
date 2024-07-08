use std::sync::{Arc, Mutex};

use waitingroom_core::{
    network::{Message, Network, NetworkHandle},
    NetworkError, NodeId,
};
use waitingroom_distributed::messages::NodeToNodeMessage;

#[derive(Debug, Clone)]
pub struct HttpNetworkProvider {
    node_id: waitingroom_core::NodeId,
    incoming_messages: Arc<Mutex<Vec<Message<NodeToNodeMessage>>>>,
    client: reqwest::Client,
}

impl HttpNetworkProvider {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            incoming_messages: Arc::new(Mutex::new(Vec::new())),
            client: reqwest::Client::new(),
        }
    }

    pub fn add_message(&mut self, message: Message<NodeToNodeMessage>) {
        self.incoming_messages.lock().unwrap().push(message);
    }
}

impl Network<NodeToNodeMessage> for HttpNetworkProvider {
    type NetworkHandle = Self;

    fn join(&self, node: waitingroom_core::NodeId) -> Result<Self::NetworkHandle, NetworkError> {
        log::info!("Node {} joined the network", node);
        Ok(self.clone())
    }

    fn all_nodes(&self) -> Result<Vec<waitingroom_core::NodeId>, NetworkError> {
        unimplemented!("unused")
    }
}

impl NetworkHandle<NodeToNodeMessage> for HttpNetworkProvider {
    fn send_message(
        &self,
        to_node: waitingroom_core::NodeId,
        message: NodeToNodeMessage,
    ) -> Result<(), NetworkError> {
        let self_node = self.node_id;
        log::info!(
            "Sending message {:?} from {} to {}",
            message,
            self_node,
            to_node
        );
        let client = self.client.clone();
        tokio::spawn(async move {
            let response = match client
                .post(format!("http://127.0.0.1:{}/msg", to_node))
                .json(&Message {
                    from_node: self_node,
                    to_node,
                    message,
                })
                .send()
                .await
            {
                Ok(response) => response,
                Err(err) => {
                    log::error!("Failed to send message to node {}", to_node);
                    log::error!("{:?}", err);
                    return;
                }
            };

            if !response.status().is_success() {
                log::error!("Failed to send message to node {}", to_node);
                log::error!("{:?}", response);
            }
        });
        Ok(())
    }

    fn receive_message(
        &mut self,
    ) -> Result<Option<waitingroom_core::network::Message<NodeToNodeMessage>>, NetworkError> {
        let mut incoming_messages = self.incoming_messages.lock().unwrap();
        if incoming_messages.is_empty() {
            return Ok(None);
        }
        let message = incoming_messages.remove(0);
        log::debug!("Processing message {:?}", message);
        Ok(Some(message))
    }
}
