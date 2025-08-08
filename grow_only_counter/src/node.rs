use maelstrom::kv::{Counter, KV};
use maelstrom::{
    Message, MessageBody,
    node::{MessageHandler, Node},
};
use std::collections::HashMap;

pub struct GrowOnlyCounterNode {
    /// Key-value store
    kv: KV,
    /// For each peer, what versions we believe they already know per node_id
    peer_known_versions: HashMap<String, HashMap<String, u64>>,
}

impl Default for GrowOnlyCounterNode {
    fn default() -> Self {
        Self::new()
    }
}

impl GrowOnlyCounterNode {
    pub fn new() -> Self {
        Self {
            kv: KV::new(),
            peer_known_versions: HashMap::new(),
        }
    }

    pub fn gossip(&mut self, node: &mut Node) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        if node.id.is_empty() || node.peers.is_empty() || self.kv.is_empty() {
            return out;
        }

        let peers = node.peers.clone();
        for peer in peers.iter() {
            let peer_versions = self.peer_known_versions.entry(peer.clone()).or_default();

            // Compute versioned delta for this peer
            let mut delta: HashMap<String, Counter> = HashMap::new();
            for (node_id, counter) in self.kv.counters.iter() {
                let known_version = peer_versions.get(node_id).copied().unwrap_or(0);
                if counter.version > known_version {
                    delta.insert(node_id.clone(), counter.clone());
                }
            }

            if delta.is_empty() {
                continue;
            }

            // Update what we believe peer knows (optimistically) to avoid resending unchanged
            for (node_id, counter) in delta.iter() {
                let entry = peer_versions.entry(node_id.clone()).or_insert(0);
                if counter.version > *entry {
                    *entry = counter.version;
                }
            }

            out.push(Message {
                src: node.id.clone(),
                dest: peer.clone(),
                body: MessageBody::CounterGossip {
                    msg_id: node.next_msg_id(),
                    counters: delta,
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

    pub fn handle_counter_gossip(&mut self, from_peer: String, counters: HashMap<String, Counter>) {
        // Merge new info into our KV
        // Clone because we also use counters to update knowledge below
        let incoming = counters.clone();
        self.kv.merge(counters);

        // Update our knowledge about what the peer knows based on their advertised versions
        let peer_versions = self.peer_known_versions.entry(from_peer).or_default();
        for (node_id, counter) in incoming.into_iter() {
            let entry = peer_versions.entry(node_id).or_insert(0);
            if counter.version > *entry {
                *entry = counter.version;
            }
        }
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
                // Pre-initialize counters for all nodes
                self.kv.init(node_ids.clone());

                // Initialize Node identity and peers
                node.handle_init(node_id.clone(), node_ids.clone());

                // Prepare per-peer known versions map
                for peer in node_ids.into_iter().filter(|n| n != &node_id) {
                    self.peer_known_versions
                        .entry(peer)
                        .or_insert_with(HashMap::new);
                }
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
                self.handle_counter_gossip(msg.src.clone(), counters);
            }
            _ => {}
        }
        out
    }
}
