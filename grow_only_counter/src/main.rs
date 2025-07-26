use maelstrom::kv::{Counter, KV};
use maelstrom::{Message, MessageBody};
use std::collections::HashMap;
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
    /// Key-value store
    kv: KV,
}

impl Node {
    fn new() -> Self {
        Self {
            id: String::new(),
            peers: Vec::new(),
            msg_id: 0,
            kv: KV::new(),
        }
    }

    fn gossip(&mut self) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        if self.id.is_empty() || self.peers.is_empty() || self.kv.is_empty() {
            return out;
        }

        self.msg_id += 1;
        for peer in self.peers.iter() {
            out.push(Message {
                src: self.id.clone(),
                dest: peer.clone(),
                body: MessageBody::CounterGossip {
                    msg_id: self.msg_id,
                    counters: self.kv.counters.clone(),
                },
            });
        }
        out
    }

    fn handle_init(&mut self, node_id: String, node_ids: Vec<String>) {
        self.id = node_id.clone();
        self.peers = node_ids.clone();
        self.peers.retain(|p| p != &self.id);
    }

    fn handle_add(&mut self, delta: u64) {
        self.kv.add(self.id.clone(), delta);
    }

    fn handle_read(&mut self) -> u64 {
        self.kv.read()
    }

    fn handle_counter_gossip(&mut self, counters: HashMap<String, Counter>) {
        self.kv.merge(counters);
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
            MessageBody::Add { msg_id, delta } => {
                self.handle_add(delta);
                out.push(Message {
                    src: self.id.clone(),
                    dest: msg.src,
                    body: MessageBody::AddOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                    },
                })
            }
            MessageBody::Read { msg_id } => {
                let value = self.handle_read();
                out.push(Message {
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
            MessageBody::CounterGossip {
                msg_id: _,
                counters,
            } => {
                self.handle_counter_gossip(counters);
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
