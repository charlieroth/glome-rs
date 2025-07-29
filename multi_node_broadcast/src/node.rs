use maelstrom::{
    Message, MessageBody,
    node::{MessageHandler, Node},
};
use rand::seq::SliceRandom;
use std::collections::HashSet;

pub struct MultiNodeBroadcastNode {
    /// Node messages
    messages: HashSet<u64>,
    /// Gossip neighbors (k-regular topology)
    gossip_peers: Vec<String>,
}

impl MultiNodeBroadcastNode {
    pub fn new() -> Self {
        Self {
            messages: HashSet::new(),
            gossip_peers: Vec::new(),
        }
    }

    pub fn construct_k_regular_neighbors(&self, node: &Node, k: usize) -> Vec<String> {
        let mut rng = rand::rng();
        let mut other_nodes: Vec<String> = node
            .peers
            .iter()
            .filter(|&peer| peer != &node.id)
            .cloned()
            .collect();

        other_nodes.shuffle(&mut rng);
        let len = other_nodes.len();
        other_nodes.into_iter().take(k.min(len)).collect()
    }

    pub fn gossip(&self, node: &mut Node) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        if node.id.is_empty() || self.gossip_peers.is_empty() || self.messages.is_empty() {
            return out;
        }

        for peer in self.gossip_peers.iter() {
            out.push(Message {
                src: node.id.clone(),
                dest: peer.clone(),
                body: MessageBody::BroadcastGossip {
                    msg_id: node.next_msg_id(),
                    messages: self.messages.iter().cloned().collect(),
                },
            });
        }
        out
    }

    pub fn handle_broadcast_gossip(&mut self, messages: Vec<u64>) {
        for message in messages {
            self.messages.insert(message);
        }
    }

    pub fn handle_broadcast(&mut self, message: u64) {
        self.messages.insert(message);
    }

    pub fn handle_read(&self) -> Vec<u64> {
        self.messages.iter().cloned().collect()
    }
}

impl MessageHandler for MultiNodeBroadcastNode {
    fn handle(&mut self, node: &mut Node, msg: Message) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        match msg.body.clone() {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                node.handle_init(node_id, node_ids);
                self.gossip_peers = self.construct_k_regular_neighbors(node, 4);
                out.push(node.init_ok(msg.src, msg_id));
            }
            MessageBody::Topology {
                msg_id,
                topology: _,
            } => {
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    msg.src,
                    MessageBody::TopologyOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                    },
                ));
            }
            MessageBody::Broadcast { msg_id, message } => {
                self.handle_broadcast(message);
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    msg.src,
                    MessageBody::BroadcastOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                    },
                ));
            }
            MessageBody::BroadcastGossip {
                msg_id: _,
                messages,
            } => {
                self.handle_broadcast_gossip(messages);
            }
            MessageBody::Read { msg_id } => {
                let messages = self.handle_read();
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    msg.src,
                    MessageBody::ReadOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                        messages: Some(messages),
                        value: None,
                    },
                ));
            }
            _ => {}
        }
        out
    }
}
