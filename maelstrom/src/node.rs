use crate::{Message, MessageBody};
use tokio::{
    io::{self, AsyncBufReadExt, BufReader},
    sync::mpsc,
};

/// Base node structure that all services can use
pub struct Node {
    /// Unique node identifier
    pub id: String,
    /// Peer node IDs for gossip
    pub peers: Vec<String>,
    /// Message counter for generating unique msg_ids
    pub msg_id: u64,
}

impl Node {
    pub fn new() -> Self {
        Self {
            id: String::new(),
            peers: Vec::new(),
            msg_id: 0,
        }
    }

    /// Handle init message and set up node identity
    pub fn handle_init(&mut self, node_id: String, node_ids: Vec<String>) {
        self.id = node_id.clone();
        self.peers = node_ids.clone();
        self.peers.retain(|p| p != &self.id);
    }

    /// Get next message ID
    pub fn next_msg_id(&mut self) -> u64 {
        self.msg_id += 1;
        self.msg_id
    }

    /// Create an InitOk response
    pub fn init_ok(&mut self, dest: String, in_reply_to: u64) -> Message {
        Message {
            src: self.id.clone(),
            dest: dest.clone(),
            body: MessageBody::InitOk {
                msg_id: self.next_msg_id(),
                in_reply_to,
            },
        }
    }

    /// Create a reply message with the given body
    pub fn reply(&mut self, dest: String, body: MessageBody) -> Message {
        Message {
            src: self.id.clone(),
            dest,
            body,
        }
    }
}

/// Trait for handling different message types
pub trait MessageHandler {
    /// Handle a message and return response messages
    fn handle(&mut self, node: &mut Node, message: Message) -> Vec<Message>;
}

/// Default message loop that reads from stdin and writes to stdout
pub async fn run_node<H: MessageHandler>(mut handler: H) {
    let mut node = Node::new();
    let (tx, mut rx) = mpsc::channel::<Message>(32);

    // Spawn stdin reader
    let stdin_tx = tx.clone();
    tokio::spawn(async move {
        let reader = BufReader::new(io::stdin());
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Ok(msg) = serde_json::from_str::<Message>(&line) {
                let _ = stdin_tx.send(msg).await;
            }
        }
    });

    // Message processing loop
    while let Some(msg) = rx.recv().await {
        for response in handler.handle(&mut node, msg) {
            let response_str = serde_json::to_string(&response).unwrap();
            println!("{response_str}");
        }
    }
}
