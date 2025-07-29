use maelstrom::kv::{Counter, KV};
use maelstrom::{
    Message, MessageBody,
    node::{MessageHandler, Node},
};
use std::collections::HashMap;
use tokio::{
    io::{self, AsyncBufReadExt, BufReader},
    sync::mpsc,
    time::{Duration, interval},
};

struct GrowOnlyCounterNode {
    /// Key-value store
    kv: KV,
}

impl GrowOnlyCounterNode {
    fn new() -> Self {
        Self {
            kv: KV::new(),
        }
    }

    fn gossip(&self, node: &mut Node) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        if node.id.is_empty() || node.peers.is_empty() || self.kv.is_empty() {
            return out;
        }

        let peers = node.peers.clone();
        for peer in peers.iter() {
            out.push(Message {
                src: node.id.clone(),
                dest: peer.clone(),
                body: MessageBody::CounterGossip {
                    msg_id: node.next_msg_id(),
                    counters: self.kv.counters.clone(),
                },
            });
        }
        out
    }

    fn handle_add(&mut self, node: &Node, delta: u64) {
        self.kv.add(node.id.clone(), delta);
    }

    fn handle_read(&self) -> u64 {
        self.kv.read()
    }

    fn handle_counter_gossip(&mut self, counters: HashMap<String, Counter>) {
        self.kv.merge(counters);
    }
}

impl MessageHandler for GrowOnlyCounterNode {
    fn handle(&mut self, node: &mut Node, msg: Message) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        match msg.body {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                node.handle_init(node_id, node_ids);
                out.push(node.init_ok(msg.src, msg_id));
            }
            MessageBody::Add { msg_id, delta } => {
                self.handle_add(node, delta);
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    msg.src,
                    MessageBody::AddOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                    },
                ));
            }
            MessageBody::Read { msg_id } => {
                let value = self.handle_read();
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    msg.src,
                    MessageBody::ReadOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                        messages: None,
                        value: Some(value),
                    },
                ));
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
    let mut handler = GrowOnlyCounterNode::new();
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
                let msgs = handler.gossip(&mut node);
                for msg in msgs {
                    let response_str = serde_json::to_string(&msg).unwrap();
                    println!("{response_str}");
                }
            }
            Some(msg) = rx.recv() => {
                for response in handler.handle(&mut node, msg) {
                    let response_str = serde_json::to_string(&response).unwrap();
                    println!("{response_str}");
                }
            }
        }
    }
}
