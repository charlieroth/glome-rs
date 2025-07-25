use maelstrom::kv::{Counter, KV};
use maelstrom::{Message, MessageBody};
use std::collections::{HashMap, HashSet};
use std::{
    io::{self, BufRead, BufReader},
    time::Duration,
};
use tokio::{sync::mpsc, time::interval};

struct Node {
    id: String,
    peers: Vec<String>,
    msg_id: u64,
    kv: KV,
    dirty: HashSet<String>,
}

impl Node {
    fn new() -> Self {
        Self {
            id: String::new(),
            peers: Vec::new(),
            msg_id: 0,
            kv: KV::new(),
            dirty: HashSet::new(),
        }
    }

    async fn gossip(&mut self) {
        if self.id.is_empty() || self.peers.is_empty() || self.kv.is_empty() {
            return;
        }

        self.msg_id += 1;
        for peer in self.peers.iter() {
            let broadcast = Message {
                src: self.id.clone(),
                dest: peer.clone(),
                body: MessageBody::CounterGossip {
                    msg_id: self.msg_id,
                    counters: self.kv.counters.clone(),
                },
            };
            let response_str = serde_json::to_string(&broadcast).unwrap();
            println!("{response_str}");
        }
    }

    async fn handle_init(&mut self, node_id: String, node_ids: Vec<String>) {
        self.id = node_id.clone();
        self.peers = node_ids.clone();
        self.peers.retain(|p| p != &self.id);
    }

    async fn handle_add(&mut self, delta: u64) {
        self.kv.add(self.id.clone(), delta);
        self.dirty.insert(self.id.clone());
    }

    async fn handle_read(&mut self) -> u64 {
        self.kv.read()
    }

    async fn handle_counter_gossip(&mut self, counters: HashMap<String, Counter>) {
        self.kv.merge(counters);
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
            MessageBody::Add { msg_id, delta } => {
                self.handle_add(delta).await;
                Some(Message {
                    src: self.id.clone(),
                    dest: msg.src,
                    body: MessageBody::AddOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                    },
                })
            }
            MessageBody::Read { msg_id } => {
                let value = self.handle_read().await;
                Some(Message {
                    src: self.id.clone(),
                    dest: msg.src,
                    body: MessageBody::ReadOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                        messages: None,
                        value: Some(value),
                    },
                })
            }
            MessageBody::CounterGossip { msg_id, counters } => {
                self.handle_counter_gossip(counters).await;
                None
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
