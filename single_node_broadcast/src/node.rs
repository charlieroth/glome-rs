use maelstrom::{
    Message, MessageBody,
    node::{MessageHandler, Node},
};
use std::collections::HashSet;

pub struct SingleNodeBroadcastNode {
    /// Node messages
    messages: Vec<u64>,
    /// Fast membership test to avoid duplicate inserts
    seen: HashSet<u64>,
}

impl Default for SingleNodeBroadcastNode {
    fn default() -> Self {
        Self::new()
    }
}

impl SingleNodeBroadcastNode {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            seen: HashSet::new(),
        }
    }

    pub fn handle_broadcast(&mut self, node: &mut Node, message: u64) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        if self.seen.insert(message) {
            self.messages.push(message);
        }
        for peer in node.peers.clone() {
            if peer == node.id {
                continue;
            }
            out.push(Message {
                src: node.id.clone(),
                dest: peer,
                body: MessageBody::Broadcast {
                    msg_id: node.next_msg_id(),
                    message,
                },
            });
        }
        out
    }

    pub fn handle_read(&self) -> Vec<u64> {
        self.messages.clone()
    }
}

impl MessageHandler for SingleNodeBroadcastNode {
    fn handle(&mut self, node: &mut Node, msg: Message) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        match msg.body.clone() {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                node.handle_init(node_id, node_ids);
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
                let broadcasts = self.handle_broadcast(node, message);
                out.extend(broadcasts);
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    msg.src,
                    MessageBody::BroadcastOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                    },
                ));
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
        let mut handler = SingleNodeBroadcastNode::new();
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
        assert_eq!(node.peers, vec!["n2", "n3"]);
    }

    #[test]
    fn test_broadcast_node_handles_topology_message() {
        let mut handler = SingleNodeBroadcastNode::new();
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
        let mut handler = SingleNodeBroadcastNode::new();
        let mut node = Node::new();

        // Initialize node with peers
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

        // Should have 3 responses: broadcasts to 2 peers + BroadcastOk to client
        assert_eq!(responses.len(), 3);

        // Check BroadcastOk response (should be last)
        let broadcast_ok = &responses[2];
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

        // Check peer broadcasts (first 2 responses)
        let peer_destinations: Vec<&String> = responses[0..2].iter().map(|msg| &msg.dest).collect();
        assert!(peer_destinations.contains(&&"n2".to_string()));
        assert!(peer_destinations.contains(&&"n3".to_string()));

        for peer_msg in &responses[0..2] {
            assert_eq!(peer_msg.src, "n1");
            match &peer_msg.body {
                MessageBody::Broadcast { msg_id: _, message } => {
                    assert_eq!(*message, 42);
                }
                _ => panic!("Expected Broadcast message to peer"),
            }
        }

        // Verify message was stored
        assert_eq!(handler.messages, vec![42]);
    }

    #[test]
    fn test_broadcast_node_handles_read_message() {
        let mut handler = SingleNodeBroadcastNode::new();
        let mut node = Node::new();

        // Initialize node
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        // Add some messages manually for testing
        handler.messages = vec![10, 20, 30];

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
                assert_eq!(messages.as_ref().unwrap(), &vec![10, 20, 30]);
                assert_eq!(*value, None);
            }
            _ => panic!("Expected ReadOk message"),
        }
    }

    #[test]
    fn test_broadcast_node_handles_multiple_broadcasts() {
        let mut handler = SingleNodeBroadcastNode::new();
        let mut node = Node::new();

        // Initialize node with one peer to simplify testing
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
        assert_eq!(responses1.len(), 2); // 1 peer broadcast + 1 BroadcastOk

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
        assert_eq!(responses2.len(), 2); // 1 peer broadcast + 1 BroadcastOk

        // Verify messages are stored in order
        assert_eq!(handler.messages, vec![100, 200]);

        // Test read to confirm both messages are returned
        let read_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Read { msg_id: 3 },
        };

        let read_responses = handler.handle(&mut node, read_message);
        match &read_responses[0].body {
            MessageBody::ReadOk { messages, .. } => {
                assert_eq!(messages.as_ref().unwrap(), &vec![100, 200]);
            }
            _ => panic!("Expected ReadOk message"),
        }
    }

    #[test]
    fn test_broadcast_node_ignores_unknown_messages() {
        let mut handler = SingleNodeBroadcastNode::new();
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
    fn test_broadcast_node_with_no_peers() {
        let mut handler = SingleNodeBroadcastNode::new();
        let mut node = Node::new();

        // Initialize node with no peers
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        let broadcast_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Broadcast {
                msg_id: 1,
                message: 42,
            },
        };

        let responses = handler.handle(&mut node, broadcast_message);

        // Should only have BroadcastOk response (no peer broadcasts)
        assert_eq!(responses.len(), 1);
        match &responses[0].body {
            MessageBody::BroadcastOk {
                msg_id: _,
                in_reply_to,
            } => {
                assert_eq!(*in_reply_to, 1);
            }
            _ => panic!("Expected BroadcastOk message"),
        }

        // Verify message was still stored
        assert_eq!(handler.messages, vec![42]);
    }

    #[test]
    fn test_broadcast_node_read_when_empty() {
        let mut handler = SingleNodeBroadcastNode::new();
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
        let mut handler = SingleNodeBroadcastNode::new();
        let mut node = Node::new();

        // Initialize node with one peer
        node.handle_init("n1".to_string(), vec!["n1".to_string(), "n2".to_string()]);

        let broadcast_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Broadcast {
                msg_id: 1,
                message: 42,
            },
        };

        let responses1 = handler.handle(&mut node, broadcast_message.clone());
        let responses2 = handler.handle(&mut node, broadcast_message);

        // Extract msg_ids from peer broadcasts (first response in each)
        let msg_id1 = match &responses1[0].body {
            MessageBody::Broadcast { msg_id, .. } => *msg_id,
            _ => panic!("Expected Broadcast message"),
        };

        let msg_id2 = match &responses2[0].body {
            MessageBody::Broadcast { msg_id, .. } => *msg_id,
            _ => panic!("Expected Broadcast message"),
        };

        assert_ne!(msg_id1, msg_id2);
    }
}
