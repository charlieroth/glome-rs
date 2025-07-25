use maelstrom::{Message, MessageBody};
use std::{
    collections::HashMap,
    io::{self, BufRead, BufReader, Write},
    sync::Arc,
};
use tokio::sync::{RwLock, mpsc};

struct Node {
    id: RwLock<String>,
    peers: RwLock<Vec<String>>,
    msg_id: RwLock<u64>,
    messages: RwLock<Vec<u64>>,
}

impl Node {
    fn new() -> Self {
        Self {
            id: RwLock::new(String::new()),
            peers: RwLock::new(Vec::new()),
            msg_id: RwLock::new(0),
            messages: RwLock::new(Vec::new()),
        }
    }

    async fn handle_init(&self, node_id: String, node_ids: Vec<String>) {
        *self.id.write().await = node_id.clone();
        *self.peers.write().await = node_ids.into_iter().filter(|id| id != &node_id).collect();
    }

    async fn handle_topology(&self, topology: HashMap<String, Vec<String>>) {
        let node_id = self.id.read().await.clone();
        if let Some(peers) = topology.get(&node_id) {
            *self.peers.write().await =
                peers.iter().filter(|id| *id != &node_id).cloned().collect();
        }
    }

    async fn handle_broadcast(&self, message: u64) {
        let stdout = io::stdout();
        let mut stdout = stdout.lock();

        self.messages.write().await.push(message);
        let peers = self.peers.read().await;
        for peer in peers.iter() {
            let broadcast = Message {
                src: self.id.read().await.clone(),
                dest: peer.clone(),
                body: MessageBody::Broadcast {
                    msg_id: *self.msg_id.read().await,
                    message,
                },
            };
            let response_str = serde_json::to_string(&broadcast).unwrap();
            writeln!(stdout, "{response_str}").unwrap();
            stdout.flush().unwrap();
            *self.msg_id.write().await += 1;
        }
    }

    async fn handle_read(&self) -> Vec<u64> {
        let messages = self.messages.read().await;
        messages.clone()
    }

    async fn process_message(&self, msg: Message) -> Option<Message> {
        *self.msg_id.write().await += 1;
        match msg.body {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                self.handle_init(node_id, node_ids).await;
                Some(Message {
                    src: self.id.read().await.clone(),
                    dest: msg.src,
                    body: MessageBody::InitOk {
                        msg_id: *self.msg_id.read().await,
                        in_reply_to: msg_id,
                    },
                })
            }
            MessageBody::Topology { msg_id, topology } => {
                self.handle_topology(topology).await;
                Some(Message {
                    src: self.id.read().await.clone(),
                    dest: msg.src,
                    body: MessageBody::TopologyOk {
                        msg_id: *self.msg_id.read().await,
                        in_reply_to: msg_id,
                    },
                })
            }
            MessageBody::Broadcast { msg_id, message } => {
                self.handle_broadcast(message).await;
                Some(Message {
                    src: self.id.read().await.clone(),
                    dest: msg.src,
                    body: MessageBody::BroadcastOk {
                        msg_id: *self.msg_id.read().await,
                        in_reply_to: msg_id,
                    },
                })
            }
            MessageBody::Read { msg_id } => {
                let messages = self.handle_read().await;
                Some(Message {
                    src: self.id.read().await.clone(),
                    dest: msg.src,
                    body: MessageBody::ReadOk {
                        msg_id: *self.msg_id.read().await,
                        in_reply_to: msg_id,
                        messages: Some(messages),
                        value: None,
                    },
                })
            }
            _ => None,
        }
    }
}

#[tokio::main]
async fn main() {
    let node = Arc::new(Node::new());
    let (tx, mut rx) = mpsc::channel::<Message>(1000);

    // Spawn stdin reader
    let stdin_tx = tx.clone();
    tokio::spawn(async move {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin);
        for line in reader.lines() {
            if let Ok(line) = line {
                eprintln!("Received: {line}");
                if let Ok(msg) = serde_json::from_str::<Message>(&line) {
                    let _ = stdin_tx.send(msg).await;
                }
            }
        }
    });

    let stdout = io::stdout();
    while let Some(msg) = rx.recv().await {
        let node_clone = node.clone();
        if let Some(response) = node_clone.process_message(msg).await {
            let response_str = serde_json::to_string(&response).unwrap();
            eprintln!("Sending: {response_str}");
            let mut stdout = stdout.lock();
            writeln!(stdout, "{response_str}").unwrap();
            stdout.flush().unwrap();
        }
    }
}
