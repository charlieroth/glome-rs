use maelstrom::{
    Message, MessageBody,
    node::{MessageHandler, Node},
};
use rand::seq::SliceRandom;
use std::collections::{HashMap, HashSet};

pub struct MultiNodeBroadcastNode {
    /// Node messages
    messages: HashSet<u64>,
    /// Gossip neighbors (k-regular topology)
    gossip_peers: Vec<String>,
    /// For each peer, the set of message ids we believe that peer already has
    peer_seen: HashMap<String, HashSet<u64>>,
}

impl Default for MultiNodeBroadcastNode {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiNodeBroadcastNode {
    pub fn new() -> Self {
        Self {
            messages: HashSet::new(),
            gossip_peers: Vec::new(),
            peer_seen: HashMap::new(),
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

    pub fn gossip(&mut self, node: &mut Node) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        if node.id.is_empty() || self.gossip_peers.is_empty() || self.messages.is_empty() {
            return out;
        }

        for peer in self.gossip_peers.iter() {
            // Compute delta: what we have that we do not believe the peer has
            let seen = self.peer_seen.entry(peer.clone()).or_default();
            let delta: Vec<u64> = self
                .messages
                .iter()
                .copied()
                .filter(|m| !seen.contains(m))
                .take(1024)
                .collect();

            if !delta.is_empty() {
                out.push(Message {
                    src: node.id.clone(),
                    dest: peer.clone(),
                    body: MessageBody::BroadcastGossip {
                        msg_id: node.next_msg_id(),
                        messages: delta,
                    },
                });
            }
        }
        out
    }

    pub fn handle_broadcast_gossip_from(&mut self, peer: &str, messages: Vec<u64>) {
        let seen = self.peer_seen.entry(peer.to_string()).or_default();
        for message in messages {
            self.messages.insert(message);
            seen.insert(message);
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
                self.handle_broadcast_gossip_from(&msg.src, messages);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_broadcast_node_handles_init_message() {
        let mut handler = MultiNodeBroadcastNode::new();
        let mut node = Node::new();

        let init_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Init {
                msg_id: 1,
                node_id: "n1".to_string(),
                node_ids: vec![
                    "n1".to_string(),
                    "n2".to_string(),
                    "n3".to_string(),
                    "n4".to_string(),
                    "n5".to_string(),
                ],
            },
        };

        let responses = handler.handle(&mut node, init_message);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n1");
        assert_eq!(responses[0].dest, "c1");

        match &responses[0].body {
            MessageBody::InitOk {
                msg_id: _,
                in_reply_to,
            } => {
                assert_eq!(*in_reply_to, 1);
            }
            _ => panic!("Expected InitOk message"),
        }

        // Verify node state was updated
        assert_eq!(node.id, "n1");
        assert_eq!(node.peers, vec!["n2", "n3", "n4", "n5"]);

        // Verify gossip peers were constructed (should be k=4, but limited by available peers)
        assert_eq!(handler.gossip_peers.len(), 4);
        for peer in &handler.gossip_peers {
            assert!(node.peers.contains(peer));
            assert_ne!(*peer, node.id);
        }
    }

    #[test]
    fn test_broadcast_node_handles_topology_message() {
        let mut handler = MultiNodeBroadcastNode::new();
        let mut node = Node::new();

        // Initialize node first
        node.handle_init("n1".to_string(), vec!["n1".to_string(), "n2".to_string()]);

        let topology_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Topology {
                msg_id: 1,
                topology: HashMap::new(),
            },
        };

        let responses = handler.handle(&mut node, topology_message);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n1");
        assert_eq!(responses[0].dest, "c1");

        match &responses[0].body {
            MessageBody::TopologyOk {
                msg_id: _,
                in_reply_to,
            } => {
                assert_eq!(*in_reply_to, 1);
            }
            _ => panic!("Expected TopologyOk message"),
        }
    }

    #[test]
    fn test_broadcast_node_handles_broadcast_message() {
        let mut handler = MultiNodeBroadcastNode::new();
        let mut node = Node::new();

        // Initialize node
        node.handle_init(
            "n1".to_string(),
            vec!["n1".to_string(), "n2".to_string(), "n3".to_string()],
        );

        let broadcast_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Broadcast {
                msg_id: 1,
                message: 42,
            },
        };

        let responses = handler.handle(&mut node, broadcast_message);

        // Should only have BroadcastOk response (no peer broadcasts in multi-node version)
        assert_eq!(responses.len(), 1);

        // Check BroadcastOk response
        let broadcast_ok = &responses[0];
        assert_eq!(broadcast_ok.src, "n1");
        assert_eq!(broadcast_ok.dest, "c1");
        match &broadcast_ok.body {
            MessageBody::BroadcastOk {
                msg_id: _,
                in_reply_to,
            } => {
                assert_eq!(*in_reply_to, 1);
            }
            _ => panic!("Expected BroadcastOk message"),
        }

        // Verify message was stored in HashSet
        assert!(handler.messages.contains(&42));
        assert_eq!(handler.messages.len(), 1);
    }

    #[test]
    fn test_broadcast_node_handles_broadcast_gossip_message() {
        let mut handler = MultiNodeBroadcastNode::new();
        let mut node = Node::new();

        // Initialize node
        node.handle_init("n1".to_string(), vec!["n1".to_string(), "n2".to_string()]);

        let gossip_message = Message {
            src: "n2".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::BroadcastGossip {
                msg_id: 1,
                messages: vec![10, 20, 30],
            },
        };

        let responses = handler.handle(&mut node, gossip_message);

        // BroadcastGossip messages don't require a response
        assert_eq!(responses.len(), 0);

        // Verify messages were stored
        assert!(handler.messages.contains(&10));
        assert!(handler.messages.contains(&20));
        assert!(handler.messages.contains(&30));
        assert_eq!(handler.messages.len(), 3);
    }

    #[test]
    fn test_broadcast_node_handles_read_message() {
        let mut handler = MultiNodeBroadcastNode::new();
        let mut node = Node::new();

        // Initialize node
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        // Add some messages manually for testing
        handler.messages.insert(10);
        handler.messages.insert(20);
        handler.messages.insert(30);

        let read_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Read { msg_id: 1 },
        };

        let responses = handler.handle(&mut node, read_message);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n1");
        assert_eq!(responses[0].dest, "c1");

        match &responses[0].body {
            MessageBody::ReadOk {
                msg_id: _,
                in_reply_to,
                messages,
                value,
            } => {
                assert_eq!(*in_reply_to, 1);
                let returned_messages = messages.as_ref().unwrap();
                assert_eq!(returned_messages.len(), 3);
                assert!(returned_messages.contains(&10));
                assert!(returned_messages.contains(&20));
                assert!(returned_messages.contains(&30));
                assert_eq!(*value, None);
            }
            _ => panic!("Expected ReadOk message"),
        }
    }

    #[test]
    fn test_gossip_method() {
        let mut handler = MultiNodeBroadcastNode::new();
        let mut node = Node::new();

        // Initialize node with peers
        node.handle_init(
            "n1".to_string(),
            vec!["n1".to_string(), "n2".to_string(), "n3".to_string()],
        );
        handler.gossip_peers = vec!["n2".to_string(), "n3".to_string()];

        // Add some messages
        handler.messages.insert(100);
        handler.messages.insert(200);

        let gossip_messages = handler.gossip(&mut node);

        assert_eq!(gossip_messages.len(), 2);

        // Check each gossip message
        for msg in &gossip_messages {
            assert_eq!(msg.src, "n1");
            assert!(msg.dest == "n2" || msg.dest == "n3");
            match &msg.body {
                MessageBody::BroadcastGossip {
                    msg_id: _,
                    messages,
                } => {
                    assert_eq!(messages.len(), 2);
                    assert!(messages.contains(&100));
                    assert!(messages.contains(&200));
                }
                _ => panic!("Expected BroadcastGossip message"),
            }
        }
    }

    #[test]
    fn test_gossip_method_with_empty_state() {
        let mut handler = MultiNodeBroadcastNode::new();
        let mut node = Node::new();

        // Test with empty node id
        let gossip_messages = handler.gossip(&mut node);
        assert_eq!(gossip_messages.len(), 0);

        // Test with empty gossip peers
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);
        let gossip_messages = handler.gossip(&mut node);
        assert_eq!(gossip_messages.len(), 0);

        // Test with empty messages
        let mut handler_with_peers = MultiNodeBroadcastNode::new();
        handler_with_peers.gossip_peers = vec!["n2".to_string()];
        let gossip_messages = handler_with_peers.gossip(&mut node);
        assert_eq!(gossip_messages.len(), 0);
    }

    #[test]
    fn test_construct_k_regular_neighbors() {
        let handler = MultiNodeBroadcastNode::new();
        let mut node = Node::new();

        // Test with 5 peers, k=3
        node.handle_init(
            "n1".to_string(),
            vec![
                "n1".to_string(),
                "n2".to_string(),
                "n3".to_string(),
                "n4".to_string(),
                "n5".to_string(),
                "n6".to_string(),
            ],
        );

        let neighbors = handler.construct_k_regular_neighbors(&node, 3);

        assert_eq!(neighbors.len(), 3);
        for neighbor in &neighbors {
            assert!(node.peers.contains(neighbor));
            assert_ne!(*neighbor, node.id);
        }

        // Test with k larger than available peers
        let large_k_neighbors = handler.construct_k_regular_neighbors(&node, 10);
        assert_eq!(large_k_neighbors.len(), 5); // Should be limited to actual peer count

        // Test with k=0
        let zero_neighbors = handler.construct_k_regular_neighbors(&node, 0);
        assert_eq!(zero_neighbors.len(), 0);
    }

    #[test]
    fn test_broadcast_node_handles_multiple_broadcasts() {
        let mut handler = MultiNodeBroadcastNode::new();
        let mut node = Node::new();

        // Initialize node
        node.handle_init("n1".to_string(), vec!["n1".to_string(), "n2".to_string()]);

        // Send first broadcast
        let broadcast1 = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Broadcast {
                msg_id: 1,
                message: 100,
            },
        };

        let responses1 = handler.handle(&mut node, broadcast1);
        assert_eq!(responses1.len(), 1); // Only BroadcastOk

        // Send second broadcast
        let broadcast2 = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Broadcast {
                msg_id: 2,
                message: 200,
            },
        };

        let responses2 = handler.handle(&mut node, broadcast2);
        assert_eq!(responses2.len(), 1); // Only BroadcastOk

        // Verify both messages are stored (HashSet ensures uniqueness)
        assert!(handler.messages.contains(&100));
        assert!(handler.messages.contains(&200));
        assert_eq!(handler.messages.len(), 2);

        // Test read to confirm both messages are returned
        let read_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Read { msg_id: 3 },
        };

        let read_responses = handler.handle(&mut node, read_message);
        match &read_responses[0].body {
            MessageBody::ReadOk { messages, .. } => {
                let returned_messages = messages.as_ref().unwrap();
                assert_eq!(returned_messages.len(), 2);
                assert!(returned_messages.contains(&100));
                assert!(returned_messages.contains(&200));
            }
            _ => panic!("Expected ReadOk message"),
        }
    }

    #[test]
    fn test_broadcast_node_deduplicates_messages() {
        let mut handler = MultiNodeBroadcastNode::new();
        let mut node = Node::new();

        // Initialize node
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        // Send same broadcast multiple times
        let broadcast_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Broadcast {
                msg_id: 1,
                message: 42,
            },
        };

        handler.handle(&mut node, broadcast_message.clone());
        handler.handle(&mut node, broadcast_message.clone());
        handler.handle(&mut node, broadcast_message);

        // Should only store one copy due to HashSet
        assert_eq!(handler.messages.len(), 1);
        assert!(handler.messages.contains(&42));
    }

    #[test]
    fn test_broadcast_node_ignores_unknown_messages() {
        let mut handler = MultiNodeBroadcastNode::new();
        let mut node = Node::new();

        let unknown_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Generate { msg_id: 1 },
        };

        let responses = handler.handle(&mut node, unknown_message);

        assert_eq!(responses.len(), 0);
    }

    #[test]
    fn test_broadcast_node_read_when_empty() {
        let mut handler = MultiNodeBroadcastNode::new();
        let mut node = Node::new();

        // Initialize node
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        let read_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Read { msg_id: 1 },
        };

        let responses = handler.handle(&mut node, read_message);

        assert_eq!(responses.len(), 1);
        match &responses[0].body {
            MessageBody::ReadOk { messages, .. } => {
                assert_eq!(messages.as_ref().unwrap(), &Vec::<u64>::new());
            }
            _ => panic!("Expected ReadOk message"),
        }
    }

    #[test]
    fn test_broadcast_node_generates_unique_msg_ids() {
        let mut handler = MultiNodeBroadcastNode::new();
        let mut node = Node::new();

        // Initialize node with gossip peers
        node.handle_init("n1".to_string(), vec!["n1".to_string(), "n2".to_string()]);
        handler.gossip_peers = vec!["n2".to_string()];
        handler.messages.insert(42);

        let gossip_messages1 = handler.gossip(&mut node);
        let gossip_messages2 = handler.gossip(&mut node);

        // Extract msg_ids from gossip messages
        let msg_id1 = match &gossip_messages1[0].body {
            MessageBody::BroadcastGossip { msg_id, .. } => *msg_id,
            _ => panic!("Expected BroadcastGossip message"),
        };

        let msg_id2 = match &gossip_messages2[0].body {
            MessageBody::BroadcastGossip { msg_id, .. } => *msg_id,
            _ => panic!("Expected BroadcastGossip message"),
        };

        assert_ne!(msg_id1, msg_id2);
    }
}
