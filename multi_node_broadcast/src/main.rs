use maelstrom::{Message, MessageBody};
use rand::seq::SliceRandom;
use std::collections::HashSet;
use tokio::{
    io::{self, AsyncBufReadExt, BufReader},
    sync::mpsc,
    time::{Duration, interval},
};

struct Node {
    /// Unique node identifier
    id: String,
    /// Peer node IDs for gossip
    peers: Vec<String>,
    /// Message counter for generating unique msg_ids
    msg_id: u64,
    /// Node messages
    messages: HashSet<u64>,
}

impl Node {
    fn new() -> Self {
        Self {
            id: String::new(),
            peers: Vec::new(),
            msg_id: 0,
            messages: HashSet::new(),
        }
    }

    fn construct_k_regular_neighbors(&self, k: usize) -> Vec<String> {
        let mut rng = rand::rng();
        let mut other_nodes: Vec<String> = self
            .peers
            .iter()
            .filter(|&node| node != &self.id)
            .cloned()
            .collect();

        other_nodes.shuffle(&mut rng);
        let len = other_nodes.len();
        other_nodes.into_iter().take(k.min(len)).collect()
    }

    fn handle_init(&mut self, node_id: String, node_ids: Vec<String>) {
        self.id = node_id.clone();
        self.peers = node_ids.clone();
        self.peers = self.construct_k_regular_neighbors(4);
    }

    fn gossip(&mut self) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        if self.id.is_empty() || self.peers.is_empty() || self.messages.is_empty() {
            return out;
        }

        self.msg_id += 1;
        for peer in self.peers.iter() {
            out.push(Message {
                src: self.id.clone(),
                dest: peer.clone(),
                body: MessageBody::BroadcastGossip {
                    msg_id: self.msg_id,
                    messages: self.messages.iter().cloned().collect(),
                },
            });
        }
        out
    }

    fn handle_broadcast_gossip(&mut self, messages: Vec<u64>) {
        for message in messages {
            self.messages.insert(message);
        }
    }

    fn handle_broadcast(&mut self, message: u64) {
        self.messages.insert(message);
    }

    fn handle_read(&self) -> Vec<u64> {
        self.messages.iter().cloned().collect()
    }

    fn process_message(&mut self, msg: Message) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        self.msg_id += 1;
        match msg.body {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                self.handle_init(node_id, node_ids);
                out.push(Message {
                    src: self.id.clone(),
                    dest: msg.src,
                    body: MessageBody::InitOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                    },
                })
            }
            MessageBody::Topology {
                msg_id,
                topology: _,
            } => out.push(Message {
                src: self.id.clone(),
                dest: msg.src,
                body: MessageBody::TopologyOk {
                    msg_id: self.msg_id,
                    in_reply_to: msg_id,
                },
            }),
            MessageBody::Broadcast { msg_id, message } => {
                self.handle_broadcast(message);
                out.push(Message {
                    src: self.id.clone(),
                    dest: msg.src,
                    body: MessageBody::BroadcastOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                    },
                })
            }
            MessageBody::BroadcastGossip {
                msg_id: _,
                messages,
            } => {
                self.handle_broadcast_gossip(messages);
            }
            MessageBody::Read { msg_id } => {
                let messages = self.handle_read();
                out.push(Message {
                    src: self.id.clone(),
                    dest: msg.src,
                    body: MessageBody::ReadOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                        messages: Some(messages),
                        value: None,
                    },
                })
            }
            _ => {}
        }
        out
    }
}

#[tokio::main]
async fn main() {
    let mut node = Node::new();
    let (tx, mut rx) = mpsc::channel::<Message>(32);
    let mut gossip_timer = interval(Duration::from_millis(100));

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

    loop {
        tokio::select! {
            _ = gossip_timer.tick() => {
                let msgs = node.gossip();
                for msg in msgs {
                    let response_str = serde_json::to_string(&msg).unwrap();
                    println!("{response_str}");
                }
            }
            Some(msg) = rx.recv() => {
                for response in node.process_message(msg) {
                    let response_str = serde_json::to_string(&response).unwrap();
                    println!("{response_str}");
                }
            }
        }
    }
}
