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

impl Default for GrowOnlyCounterNode {
    fn default() -> Self {
        Self::new()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grow_only_counter_node_handles_init_message() {
        let mut handler = GrowOnlyCounterNode::new();
        let mut node = Node::new();
        
        let init_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Init {
                msg_id: 1,
                node_id: "n1".to_string(),
                node_ids: vec!["n1".to_string(), "n2".to_string(), "n3".to_string()],
            },
        };

        let responses = handler.handle(&mut node, init_message);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n1");
        assert_eq!(responses[0].dest, "c1");
        
        match &responses[0].body {
            MessageBody::InitOk { msg_id: _, in_reply_to } => {
                assert_eq!(*in_reply_to, 1);
            }
            _ => panic!("Expected InitOk message"),
        }

        // Verify node state was updated
        assert_eq!(node.id, "n1");
        assert_eq!(node.peers, vec!["n2", "n3"]);
    }

    #[test]
    fn test_grow_only_counter_node_handles_add_message() {
        let mut handler = GrowOnlyCounterNode::new();
        let mut node = Node::new();
        
        // Initialize node first
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        let add_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Add {
                msg_id: 42,
                delta: 10,
            },
        };

        let responses = handler.handle(&mut node, add_message);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n1");
        assert_eq!(responses[0].dest, "c1");
        
        match &responses[0].body {
            MessageBody::AddOk { msg_id: _, in_reply_to } => {
                assert_eq!(*in_reply_to, 42);
            }
            _ => panic!("Expected AddOk message"),
        }

        // Verify counter was updated
        assert_eq!(handler.handle_read(), 10);
    }

    #[test]
    fn test_grow_only_counter_node_handles_read_message() {
        let mut handler = GrowOnlyCounterNode::new();
        let mut node = Node::new();
        
        // Initialize node first
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        // Add some value first
        handler.handle_add(&node, 25);

        let read_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Read { msg_id: 99 },
        };

        let responses = handler.handle(&mut node, read_message);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n1");
        assert_eq!(responses[0].dest, "c1");
        
        match &responses[0].body {
            MessageBody::ReadOk { msg_id: _, in_reply_to, messages, value } => {
                assert_eq!(*in_reply_to, 99);
                assert_eq!(*messages, None);
                assert_eq!(*value, Some(25));
            }
            _ => panic!("Expected ReadOk message"),
        }
    }

    #[test]
    fn test_grow_only_counter_node_handles_counter_gossip() {
        let mut handler = GrowOnlyCounterNode::new();
        let mut node = Node::new();
        
        // Initialize node first
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        // Add local value
        handler.handle_add(&node, 5);

        // Create gossip counters from other nodes
        let mut gossip_counters = HashMap::new();
        gossip_counters.insert("n2".to_string(), Counter { version: 1, value: 10 });
        gossip_counters.insert("n3".to_string(), Counter { version: 1, value: 15 });

        let gossip_message = Message {
            src: "n2".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::CounterGossip {
                msg_id: 123,
                counters: gossip_counters,
            },
        };

        let responses = handler.handle(&mut node, gossip_message);

        // Gossip messages don't generate responses
        assert_eq!(responses.len(), 0);

        // Verify counters were merged (5 + 10 + 15 = 30)
        assert_eq!(handler.handle_read(), 30);
    }

    #[test]
    fn test_grow_only_counter_node_ignores_unknown_messages() {
        let mut handler = GrowOnlyCounterNode::new();
        let mut node = Node::new();
        
        let unknown_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Echo { msg_id: 1, echo: "test".to_string() },
        };

        let responses = handler.handle(&mut node, unknown_message);

        assert_eq!(responses.len(), 0);
    }

    #[test]
    fn test_multiple_add_operations() {
        let mut handler = GrowOnlyCounterNode::new();
        let mut node = Node::new();
        
        // Initialize node first
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        // Perform multiple add operations
        let add1 = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Add { msg_id: 1, delta: 5 },
        };

        let add2 = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Add { msg_id: 2, delta: 10 },
        };

        let add3 = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Add { msg_id: 3, delta: 3 },
        };

        let responses1 = handler.handle(&mut node, add1);
        let responses2 = handler.handle(&mut node, add2);
        let responses3 = handler.handle(&mut node, add3);

        // All should respond with AddOk
        assert_eq!(responses1.len(), 1);
        assert_eq!(responses2.len(), 1);
        assert_eq!(responses3.len(), 1);

        // Total should be 5 + 10 + 3 = 18
        assert_eq!(handler.handle_read(), 18);
    }

    #[test]
    fn test_gossip_functionality() {
        let mut handler = GrowOnlyCounterNode::new();
        let mut node = Node::new();
        
        // Initialize node with peers
        node.handle_init("n1".to_string(), vec!["n1".to_string(), "n2".to_string(), "n3".to_string()]);

        // Add some value to create gossip content
        handler.handle_add(&node, 42);

        let gossip_messages = handler.gossip(&mut node);

        assert_eq!(gossip_messages.len(), 2); // Should send to n2 and n3
        
        for msg in &gossip_messages {
            assert_eq!(msg.src, "n1");
            assert!(msg.dest == "n2" || msg.dest == "n3");
            
            match &msg.body {
                MessageBody::CounterGossip { msg_id: _, counters } => {
                    assert!(counters.contains_key("n1"));
                    assert_eq!(counters.get("n1").unwrap().value, 42);
                }
                _ => panic!("Expected CounterGossip message"),
            }
        }
    }

    #[test]
    fn test_gossip_empty_conditions() {
        let handler = GrowOnlyCounterNode::new();
        let mut node = Node::new();

        // Test with uninitialized node (empty id)
        let gossip_messages = handler.gossip(&mut node);
        assert_eq!(gossip_messages.len(), 0);

        // Initialize but with no peers
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);
        let gossip_messages = handler.gossip(&mut node);
        assert_eq!(gossip_messages.len(), 0);

        // Add peers but no counter data
        node.handle_init("n1".to_string(), vec!["n1".to_string(), "n2".to_string()]);
        let gossip_messages = handler.gossip(&mut node);
        assert_eq!(gossip_messages.len(), 0); // Empty KV should not gossip
    }

    #[test]
    fn test_crdt_merge_behavior() {
        let mut handler = GrowOnlyCounterNode::new();
        let mut node = Node::new();
        
        // Initialize node
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        // Add local counter value
        handler.handle_add(&node, 10);
        assert_eq!(handler.handle_read(), 10);

        // Merge with higher value from same node (should take max)
        let mut counters1 = HashMap::new();
        counters1.insert("n1".to_string(), Counter { version: 2, value: 15 });
        handler.handle_counter_gossip(counters1);
        assert_eq!(handler.handle_read(), 15);

        // Merge with lower value from same node (should keep higher)
        let mut counters2 = HashMap::new();
        counters2.insert("n1".to_string(), Counter { version: 1, value: 8 });
        handler.handle_counter_gossip(counters2);
        assert_eq!(handler.handle_read(), 15);

        // Add counters from different nodes
        let mut counters3 = HashMap::new();
        counters3.insert("n2".to_string(), Counter { version: 1, value: 20 });
        counters3.insert("n3".to_string(), Counter { version: 1, value: 5 });
        handler.handle_counter_gossip(counters3);
        assert_eq!(handler.handle_read(), 40); // 15 + 20 + 5
    }

    #[test]
    fn test_generates_unique_msg_ids() {
        let mut handler = GrowOnlyCounterNode::new();
        let mut node = Node::new();
        
        // Initialize node first
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        let add_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Add { msg_id: 1, delta: 5 },
        };

        let responses1 = handler.handle(&mut node, add_message.clone());
        let responses2 = handler.handle(&mut node, add_message);

        // Extract msg_ids from responses
        let msg_id1 = match &responses1[0].body {
            MessageBody::AddOk { msg_id, .. } => *msg_id,
            _ => panic!("Expected AddOk message"),
        };

        let msg_id2 = match &responses2[0].body {
            MessageBody::AddOk { msg_id, .. } => *msg_id,
            _ => panic!("Expected AddOk message"),
        };

        assert_ne!(msg_id1, msg_id2);
        assert_eq!(msg_id2, msg_id1 + 1);
    }
}
