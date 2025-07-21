use crate::{Body, Envelope, InitOk};
use tokio::sync::mpsc;

/// Base node structure with common fields across all Maelstrom services
#[derive(Debug)]
pub struct Node {
    pub msg_id: u64,
    pub node_id: String,
    pub node_ids: Vec<String>,
    pub sender: mpsc::Sender<Envelope>,
    pub receiver: mpsc::Receiver<Envelope>,
}

impl Node {
    /// Creates a new Node instance with default values
    pub fn new(sender: mpsc::Sender<Envelope>, receiver: mpsc::Receiver<Envelope>) -> Self {
        Self {
            msg_id: 0,
            node_id: String::new(),
            node_ids: Vec::new(),
            sender,
            receiver,
        }
    }

    /// Increments and returns the next message ID
    pub fn next_msg_id(&mut self) -> u64 {
        self.msg_id += 1;
        self.msg_id
    }

    /// Handles the Init message and updates node state
    pub async fn handle_init(
        &mut self,
        init: crate::Init,
        src: String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let msg_id = self.next_msg_id();
        self.node_id = init.node_id;
        self.node_ids = init.node_ids;

        self.sender
            .send(Envelope {
                src: self.node_id.clone(),
                dest: src,
                body: Body::InitOk(InitOk {
                    msg_id,
                    in_reply_to: init.msg_id,
                }),
            })
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    /// Sends an envelope through the sender channel
    pub async fn send(
        &self,
        envelope: Envelope,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.sender
            .send(envelope)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    /// Receives the next envelope from the receiver channel
    pub async fn recv(&mut self) -> Option<Envelope> {
        self.receiver.recv().await
    }
}

/// Trait for extending Node with service-specific behavior
pub trait NodeExt {
    /// Run the main message loop for the service
    fn run(&mut self) -> impl Future<Output = ()>;
}
