use maelstrom::{Message, MessageBody};
use rand::seq::SliceRandom;
use std::{
    collections::HashSet,
    io::{self, BufRead, BufReader},
    time::Duration,
};
use tokio::{sync::mpsc, time::interval};

struct Node {
    id: String,
    peers: Vec<String>,
    msg_id: u64,
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

    async fn construct_k_regular_neighbors(&self, k: usize) -> Vec<String> {
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

    async fn handle_init(&mut self, node_id: String, node_ids: Vec<String>) {
        self.id = node_id.clone();
        self.peers = node_ids.clone();
        self.peers = self.construct_k_regular_neighbors(4).await;
    }

    async fn gossip(&mut self) {
        if self.id.is_empty() || self.peers.is_empty() || self.messages.is_empty() {
            return;
        }

        self.msg_id += 1;
        for peer in self.peers.iter() {
            let broadcast = Message {
                src: self.id.clone(),
                dest: peer.clone(),
                body: MessageBody::BroadcastGossip {
                    msg_id: self.msg_id,
                    messages: self.messages.iter().cloned().collect(),
                },
            };
            let response_str = serde_json::to_string(&broadcast).unwrap();
            println!("{response_str}");
        }
    }

    async fn handle_broadcast_gossip(&mut self, messages: Vec<u64>) {
        for message in messages {
            self.messages.insert(message);
        }
    }

    async fn handle_broadcast(&mut self, message: u64) {
        self.messages.insert(message);
    }

    async fn handle_read(&self) -> Vec<u64> {
        self.messages.iter().cloned().collect()
    }

    async fn process_message(&mut self, msg: Message) -> Option<Message> {
        self.msg_id += 1;
        match msg.body {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                self.handle_init(node_id, node_ids).await;
                Some(Message {
                    src: self.id.clone(),
                    dest: msg.src,
                    body: MessageBody::InitOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                    },
                })
            }
            MessageBody::Topology { msg_id, topology } => Some(Message {
                src: self.id.clone(),
                dest: msg.src,
                body: MessageBody::TopologyOk {
                    msg_id: self.msg_id,
                    in_reply_to: msg_id,
                },
            }),
            MessageBody::Broadcast { msg_id, message } => {
                self.handle_broadcast(message).await;
                Some(Message {
                    src: self.id.clone(),
                    dest: msg.src,
                    body: MessageBody::BroadcastOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                    },
                })
            }
            MessageBody::BroadcastGossip { msg_id, messages } => {
                self.handle_broadcast_gossip(messages).await;
                None
            }
            MessageBody::Read { msg_id } => {
                let messages = self.handle_read().await;
                Some(Message {
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
            _ => None,
        }
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
        let stdin = io::stdin();
        let reader = BufReader::new(stdin);
        for line in reader.lines() {
            if let Ok(line) = line {
                if let Ok(msg) = serde_json::from_str::<Message>(&line) {
                    let _ = stdin_tx.send(msg).await;
                }
            }
        }
    });

    loop {
        tokio::select! {
            _ = gossip_timer.tick() => {
                node.gossip().await;
            }
            Some(msg) = rx.recv() => {
                if let Some(response) = node.process_message(msg).await {
                    let response_str = serde_json::to_string(&response).unwrap();
                    println!("{response_str}");
                }
            }
        }
    }
}
