use maelstrom::kv::{Counter, KV};
use maelstrom::{
    Message, MessageBody,
    node::{MessageHandler, Node},
};
use std::collections::HashMap;

pub struct GrowOnlyCounterNode {
    /// Key-value store
    kv: KV,
}

impl GrowOnlyCounterNode {
    pub fn new() -> Self {
        Self { kv: KV::new() }
    }

    pub fn gossip(&self, node: &mut Node) -> Vec<Message> {
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

    pub fn handle_add(&mut self, node: &Node, delta: u64) {
        self.kv.add(node.id.clone(), delta);
    }

    pub fn handle_read(&self) -> u64 {
        self.kv.read()
    }

    pub fn handle_counter_gossip(&mut self, counters: HashMap<String, Counter>) {
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
