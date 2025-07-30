use maelstrom::simple_log::Logs;
use maelstrom::{
    Message, MessageBody,
    node::{MessageHandler, Node},
};

pub struct KafkaNode {
    /// Append-only logs
    logs: Logs,
}

impl Default for KafkaNode {
    fn default() -> Self {
        Self::new()
    }
}

impl KafkaNode {
    pub fn new() -> Self {
        Self { logs: Logs::new() }
    }
}

impl MessageHandler for KafkaNode {
    fn handle(&mut self, node: &mut Node, message: Message) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        match message.body.clone() {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                node.handle_init(node_id, node_ids);
                out.push(node.init_ok(message.src, msg_id));
            }
            MessageBody::Send { msg_id, key, msg } => {
                let offset = self.logs.append(&key, msg);
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    message.src,
                    MessageBody::SendOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                        offset,
                    },
                ));
            }
            MessageBody::Poll { msg_id, offsets } => {
                let msgs = self.logs.poll(&offsets);
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    message.src,
                    MessageBody::PollOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                        msgs,
                    },
                ));
            }
            MessageBody::CommitOffsets { msg_id, offsets } => {
                self.logs.commit_offsets(offsets);
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    message.src,
                    MessageBody::CommitOffsetsOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                    },
                ));
            }
            MessageBody::ListCommittedOffsets { msg_id, keys } => {
                let offsets = self.logs.list_committed_offsets(&keys);
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    message.src,
                    MessageBody::ListCommittedOffsetsOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                        offsets,
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
    fn test_kafka_node_handles_init_message() {
        let mut handler = KafkaNode::new();
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
    fn test_kafka_node_handles_send_message() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();
        
        // Initialize node first
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        let send_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Send {
                msg_id: 42,
                key: "k1".to_string(),
                msg: 123,
            },
        };

        let responses = handler.handle(&mut node, send_message);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n1");
        assert_eq!(responses[0].dest, "c1");
        
        match &responses[0].body {
            MessageBody::SendOk { msg_id: _, in_reply_to, offset } => {
                assert_eq!(*in_reply_to, 42);
                assert_eq!(*offset, 0); // First message should have offset 0
            }
            _ => panic!("Expected SendOk message"),
        }
    }

    #[test]
    fn test_kafka_node_handles_multiple_send_messages() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();
        
        // Initialize node first
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        // Send first message to k1
        let send1 = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Send {
                msg_id: 1,
                key: "k1".to_string(),
                msg: 123,
            },
        };

        // Send second message to k1
        let send2 = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Send {
                msg_id: 2,
                key: "k1".to_string(),
                msg: 456,
            },
        };

        // Send message to different key k2
        let send3 = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Send {
                msg_id: 3,
                key: "k2".to_string(),
                msg: 789,
            },
        };

        let responses1 = handler.handle(&mut node, send1);
        let responses2 = handler.handle(&mut node, send2);
        let responses3 = handler.handle(&mut node, send3);

        // Verify offsets are increasing for same key
        match &responses1[0].body {
            MessageBody::SendOk { offset, .. } => assert_eq!(*offset, 0),
            _ => panic!("Expected SendOk message"),
        }

        match &responses2[0].body {
            MessageBody::SendOk { offset, .. } => assert_eq!(*offset, 1),
            _ => panic!("Expected SendOk message"),
        }

        // Different key should start from 0
        match &responses3[0].body {
            MessageBody::SendOk { offset, .. } => assert_eq!(*offset, 0),
            _ => panic!("Expected SendOk message"),
        }
    }

    #[test]
    fn test_kafka_node_handles_poll_message() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();
        
        // Initialize node first
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        // Send some messages first
        let send1 = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Send {
                msg_id: 1,
                key: "k1".to_string(),
                msg: 123,
            },
        };

        let send2 = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Send {
                msg_id: 2,
                key: "k2".to_string(),
                msg: 456,
            },
        };

        handler.handle(&mut node, send1);
        handler.handle(&mut node, send2);

        // Now poll for messages
        let mut poll_offsets = HashMap::new();
        poll_offsets.insert("k1".to_string(), 0);
        poll_offsets.insert("k2".to_string(), 0);

        let poll_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Poll {
                msg_id: 10,
                offsets: poll_offsets,
            },
        };

        let responses = handler.handle(&mut node, poll_message);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n1");
        assert_eq!(responses[0].dest, "c1");
        
        match &responses[0].body {
            MessageBody::PollOk { msg_id: _, in_reply_to, msgs } => {
                assert_eq!(*in_reply_to, 10);
                assert!(msgs.contains_key("k1"));
                assert!(msgs.contains_key("k2"));
                
                // Check that we got the messages back
                let k1_msgs = &msgs["k1"];
                let k2_msgs = &msgs["k2"];
                
                assert_eq!(k1_msgs.len(), 1);
                assert_eq!(k1_msgs[0], (0, 123));
                
                assert_eq!(k2_msgs.len(), 1);
                assert_eq!(k2_msgs[0], (0, 456));
            }
            _ => panic!("Expected PollOk message"),
        }
    }

    #[test]
    fn test_kafka_node_handles_commit_offsets_message() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();
        
        // Initialize node first
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        let mut commit_offsets = HashMap::new();
        commit_offsets.insert("k1".to_string(), 1000);
        commit_offsets.insert("k2".to_string(), 2000);

        let commit_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::CommitOffsets {
                msg_id: 42,
                offsets: commit_offsets,
            },
        };

        let responses = handler.handle(&mut node, commit_message);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n1");
        assert_eq!(responses[0].dest, "c1");
        
        match &responses[0].body {
            MessageBody::CommitOffsetsOk { msg_id: _, in_reply_to } => {
                assert_eq!(*in_reply_to, 42);
            }
            _ => panic!("Expected CommitOffsetsOk message"),
        }
    }

    #[test]
    fn test_kafka_node_handles_list_committed_offsets_message() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();
        
        // Initialize node first
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        // First send messages to create the logs
        let send1 = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Send {
                msg_id: 1,
                key: "k1".to_string(),
                msg: 123,
            },
        };

        let send2 = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Send {
                msg_id: 2,
                key: "k2".to_string(),
                msg: 456,
            },
        };

        handler.handle(&mut node, send1);
        handler.handle(&mut node, send2);

        // Now commit some offsets
        let mut commit_offsets = HashMap::new();
        commit_offsets.insert("k1".to_string(), 0);
        commit_offsets.insert("k2".to_string(), 0);

        let commit_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::CommitOffsets {
                msg_id: 3,
                offsets: commit_offsets,
            },
        };

        handler.handle(&mut node, commit_message);

        // Now list committed offsets
        let list_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::ListCommittedOffsets {
                msg_id: 10,
                keys: vec!["k1".to_string(), "k2".to_string(), "k3".to_string()],
            },
        };

        let responses = handler.handle(&mut node, list_message);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n1");
        assert_eq!(responses[0].dest, "c1");
        
        match &responses[0].body {
            MessageBody::ListCommittedOffsetsOk { msg_id: _, in_reply_to, offsets } => {
                assert_eq!(*in_reply_to, 10);
                assert_eq!(offsets.get("k1"), Some(&0));
                assert_eq!(offsets.get("k2"), Some(&0));
                // k3 should not be present since it wasn't committed
                assert_eq!(offsets.get("k3"), None);
            }
            _ => panic!("Expected ListCommittedOffsetsOk message"),
        }
    }

    #[test]
    fn test_kafka_node_ignores_unknown_messages() {
        let mut handler = KafkaNode::new();
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
    fn test_kafka_node_generates_unique_msg_ids() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();
        
        // Initialize node first
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        let send_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Send {
                msg_id: 1,
                key: "k1".to_string(),
                msg: 123,
            },
        };

        let responses1 = handler.handle(&mut node, send_message.clone());
        let responses2 = handler.handle(&mut node, send_message);

        // Extract msg_ids from responses
        let msg_id1 = match &responses1[0].body {
            MessageBody::SendOk { msg_id, .. } => *msg_id,
            _ => panic!("Expected SendOk message"),
        };

        let msg_id2 = match &responses2[0].body {
            MessageBody::SendOk { msg_id, .. } => *msg_id,
            _ => panic!("Expected SendOk message"),
        };

        assert_ne!(msg_id1, msg_id2);
        assert_eq!(msg_id2, msg_id1 + 1);
    }

    #[test]
    fn test_kafka_node_full_workflow() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();
        
        // Initialize node
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        // Send multiple messages
        for i in 0..3 {
            let send_message = Message {
                src: "c1".to_string(),
                dest: "n1".to_string(),
                body: MessageBody::Send {
                    msg_id: i,
                    key: "test-key".to_string(),
                    msg: 100 + i,
                },
            };
            handler.handle(&mut node, send_message);
        }

        // Poll for messages
        let mut poll_offsets = HashMap::new();
        poll_offsets.insert("test-key".to_string(), 0);

        let poll_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Poll {
                msg_id: 10,
                offsets: poll_offsets,
            },
        };

        let poll_responses = handler.handle(&mut node, poll_message);
        
        match &poll_responses[0].body {
            MessageBody::PollOk { msgs, .. } => {
                let test_key_msgs = &msgs["test-key"];
                assert_eq!(test_key_msgs.len(), 3);
                assert_eq!(test_key_msgs[0], (0, 100));
                assert_eq!(test_key_msgs[1], (1, 101));
                assert_eq!(test_key_msgs[2], (2, 102));
            }
            _ => panic!("Expected PollOk message"),
        }

        // Commit offsets
        let mut commit_offsets = HashMap::new();
        commit_offsets.insert("test-key".to_string(), 2);

        let commit_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::CommitOffsets {
                msg_id: 20,
                offsets: commit_offsets,
            },
        };

        handler.handle(&mut node, commit_message);

        // List committed offsets
        let list_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::ListCommittedOffsets {
                msg_id: 30,
                keys: vec!["test-key".to_string()],
            },
        };

        let list_responses = handler.handle(&mut node, list_message);
        
        match &list_responses[0].body {
            MessageBody::ListCommittedOffsetsOk { offsets, .. } => {
                assert_eq!(offsets.get("test-key"), Some(&2));
            }
            _ => panic!("Expected ListCommittedOffsetsOk message"),
        }
    }
}
