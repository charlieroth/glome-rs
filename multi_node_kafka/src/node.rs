use maelstrom::log::Logs;
use maelstrom::{
    Message, MessageBody,
    node::{MessageHandler, Node},
};
use std::collections::{HashMap, HashSet};

pub struct Pending {
    client: String,
    client_msg_id: u64,
    acks: usize,
    /// Set of replica node IDs that have acked this offset (seeded with leader)
    from: HashSet<String>,
}

pub struct KafkaNode {
    /// Current leader node ID in the cluster
    leader: String,
    /// Next offset for node to use
    next_offset: u64,
    /// Append-only logs
    logs: Logs,
    /// Pending operations
    pendings: HashMap<u64, Pending>,
}

impl Default for KafkaNode {
    fn default() -> Self {
        Self::new()
    }
}

impl KafkaNode {
    pub fn new() -> Self {
        Self {
            leader: String::new(),
            next_offset: 0,
            logs: Logs::new(),
            pendings: HashMap::new(),
        }
    }

    pub fn quorum(&self, node: &Node) -> usize {
        node.peers.len().div_ceil(2) + 1
    }

    pub fn handle_init(&mut self, node: &mut Node, node_id: String, node_ids: Vec<String>) {
        node.handle_init(node_id.clone(), node_ids.clone());
        let mut all = node_ids.clone();
        all.sort();
        self.leader = all[0].clone();
    }

    pub fn handle_send(
        &mut self,
        node: &mut Node,
        message: Message,
        msg_id: u64,
        key: String,
        msg: u64,
    ) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        if node.id != self.leader {
            out.push(Message {
                src: node.id.clone(),
                dest: self.leader.clone(),
                body: MessageBody::ForwardSend {
                    msg_id: node.next_msg_id(),
                    orig_src: message.src,
                    orig_msg_id: msg_id,
                    key,
                    msg,
                },
            })
        } else {
            let offset = self.logs.append_local(&key, msg);
            self.next_offset = offset + 1;
            self.pendings.insert(
                offset,
                Pending {
                    client: message.src.clone(),
                    client_msg_id: msg_id,
                    acks: 1,
                    from: HashSet::from([node.id.clone()]),
                },
            );
            let peers = node.peers.clone();
            for peer in peers {
                let msg_id = node.next_msg_id();
                out.push(Message {
                    src: node.id.clone(),
                    dest: peer,
                    body: MessageBody::Replicate {
                        msg_id,
                        key: key.clone(),
                        msg,
                        offset,
                    },
                })
            }
            if self.quorum(node) <= 1 {
                out.push(Message {
                    src: node.id.clone(),
                    dest: message.src,
                    body: MessageBody::SendOk {
                        msg_id: node.next_msg_id(),
                        in_reply_to: msg_id,
                        offset,
                    },
                });
                self.pendings.remove(&offset);
            }
        }
        out
    }
}

impl MessageHandler for KafkaNode {
    fn handle(&mut self, node: &mut Node, message: Message) -> Vec<Message> {
        let mut out = Vec::new();
        match message.body.clone() {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                self.handle_init(node, node_id, node_ids);
                out.push(node.init_ok(message.src, msg_id));
            }
            MessageBody::Send { msg_id, key, msg } => {
                let msgs = self.handle_send(node, message.clone(), msg_id, key.clone(), msg);
                out.extend(msgs);
            }
            MessageBody::ForwardSend {
                msg_id: _,
                orig_src,
                orig_msg_id,
                key,
                msg,
            } => {
                // leader handles forwarded same as `Send`
                // reuse above by recursive call
                let fwd = Message {
                    src: orig_src,
                    dest: node.id.clone(),
                    body: MessageBody::Send {
                        msg_id: orig_msg_id,
                        key,
                        msg,
                    },
                };
                out.extend(self.handle(node, fwd));
            }
            MessageBody::Replicate {
                msg_id,
                key,
                msg,
                offset,
            } => {
                self.logs.insert_at(&key, offset, msg);
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    message.src,
                    MessageBody::ReplicateOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                        offset,
                    },
                ))
            }
            MessageBody::ReplicateOk {
                msg_id: _,
                in_reply_to: _,
                offset,
            } => {
                // Grab quorum once, before get_mut()
                let quorum = self.quorum(node);
                // Mutably borrow the pending entry and bump acks only on first ack from this src
                if let Some(p) = self.pendings.get_mut(&offset) {
                    if p.from.insert(message.src.clone()) {
                        p.acks += 1;
                        // Check against the pre-computed quorum
                        if p.acks >= quorum {
                            // Take ownership of the Pending so we drop the &mut borrow
                            let Pending {
                                client,
                                client_msg_id,
                                ..
                            } = self.pendings.remove(&offset).unwrap();
                            // Now safe to immutably borrow `self` to build the response
                            let reply_msg_id = node.next_msg_id();
                            out.push(node.reply(
                                client,
                                MessageBody::SendOk {
                                    msg_id: reply_msg_id,
                                    in_reply_to: client_msg_id,
                                    offset,
                                },
                            ));
                        }
                    }
                }
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
                ))
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
                ))
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
                ))
            }
            _ => {}
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn test_kafka_node_handles_init_message() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();

        let init_message = Message {
            src: "c1".to_string(),
            dest: "n2".to_string(),
            body: MessageBody::Init {
                msg_id: 1,
                node_id: "n2".to_string(),
                node_ids: vec!["n1".to_string(), "n2".to_string(), "n3".to_string()],
            },
        };

        let responses = handler.handle(&mut node, init_message);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n2");
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
        assert_eq!(node.id, "n2");
        assert_eq!(node.peers, vec!["n1", "n3"]);

        // Verify leader is set to first node alphabetically
        assert_eq!(handler.leader, "n1");
    }

    #[test]
    fn test_leader_election_logic() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();

        // Test with different ordering - leader should always be alphabetically first
        let node_ids = vec!["n3".to_string(), "n1".to_string(), "n2".to_string()];
        handler.handle_init(&mut node, "n2".to_string(), node_ids);
        assert_eq!(handler.leader, "n1");

        // Test with single node
        let mut handler2 = KafkaNode::new();
        let mut node2 = Node::new();
        handler2.handle_init(&mut node2, "n1".to_string(), vec!["n1".to_string()]);
        assert_eq!(handler2.leader, "n1");

        // Test with different alphabetical ordering
        let mut handler3 = KafkaNode::new();
        let mut node3 = Node::new();
        let node_ids = vec!["zebra".to_string(), "alpha".to_string(), "beta".to_string()];
        handler3.handle_init(&mut node3, "beta".to_string(), node_ids);
        assert_eq!(handler3.leader, "alpha");
    }

    #[test]
    fn test_quorum_calculation() {
        let handler = KafkaNode::new();
        let mut node = Node::new();

        // Single node cluster: quorum = 1
        node.peers = vec![];
        assert_eq!(handler.quorum(&node), 1);

        // 3 node cluster: quorum = 2
        node.peers = vec!["n2".to_string(), "n3".to_string()];
        assert_eq!(handler.quorum(&node), 2);

        // 5 node cluster: quorum = 3
        node.peers = vec![
            "n2".to_string(),
            "n3".to_string(),
            "n4".to_string(),
            "n5".to_string(),
        ];
        assert_eq!(handler.quorum(&node), 3);

        // 4 node cluster: quorum = 3
        node.peers = vec!["n2".to_string(), "n3".to_string(), "n4".to_string()];
        assert_eq!(handler.quorum(&node), 3);
    }

    #[test]
    fn test_leader_handles_send_message_single_node() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();

        // Initialize as leader in single-node cluster
        handler.handle_init(&mut node, "n1".to_string(), vec!["n1".to_string()]);

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

        // Single node cluster should respond immediately (quorum = 1)
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n1");
        assert_eq!(responses[0].dest, "c1");

        match &responses[0].body {
            MessageBody::SendOk {
                msg_id: _,
                in_reply_to,
                offset,
            } => {
                assert_eq!(*in_reply_to, 42);
                assert_eq!(*offset, 0);
            }
            _ => panic!("Expected SendOk message"),
        }

        // No pending operations should remain
        assert_eq!(handler.pendings.len(), 0);
    }

    #[test]
    fn test_leader_handles_send_message_multi_node() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();

        // Initialize as leader in 3-node cluster
        handler.handle_init(
            &mut node,
            "n1".to_string(),
            vec!["n1".to_string(), "n2".to_string(), "n3".to_string()],
        );

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

        // Should send replication messages to peers, no immediate response (quorum = 2)
        assert_eq!(responses.len(), 2);

        // Check replication messages
        for response in responses.iter() {
            assert_eq!(response.src, "n1");
            assert!(response.dest == "n2" || response.dest == "n3");
            match &response.body {
                MessageBody::Replicate {
                    msg_id: _,
                    key,
                    msg,
                    offset,
                } => {
                    assert_eq!(key, "k1");
                    assert_eq!(*msg, 123);
                    assert_eq!(*offset, 0);
                }
                _ => panic!("Expected Replicate message"),
            }
        }

        // Should have pending operation
        assert_eq!(handler.pendings.len(), 1);
        let pending = handler.pendings.get(&0).unwrap();
        assert_eq!(pending.client, "c1");
        assert_eq!(pending.client_msg_id, 42);
        assert_eq!(pending.acks, 1);
    }

    #[test]
    fn test_non_leader_forwards_send_message() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();

        // Initialize as non-leader in 3-node cluster
        handler.handle_init(
            &mut node,
            "n2".to_string(),
            vec!["n1".to_string(), "n2".to_string(), "n3".to_string()],
        );

        let send_message = Message {
            src: "c1".to_string(),
            dest: "n2".to_string(),
            body: MessageBody::Send {
                msg_id: 42,
                key: "k1".to_string(),
                msg: 123,
            },
        };

        let responses = handler.handle(&mut node, send_message);

        // Should forward to leader
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n2");
        assert_eq!(responses[0].dest, "n1"); // Leader

        match &responses[0].body {
            MessageBody::ForwardSend {
                msg_id: _,
                orig_src,
                orig_msg_id,
                key,
                msg,
            } => {
                assert_eq!(orig_src, "c1");
                assert_eq!(*orig_msg_id, 42);
                assert_eq!(key, "k1");
                assert_eq!(*msg, 123);
            }
            _ => panic!("Expected ForwardSend message"),
        }
    }

    #[test]
    fn test_leader_handles_forward_send_message() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();

        // Initialize as leader in 3-node cluster
        handler.handle_init(
            &mut node,
            "n1".to_string(),
            vec!["n1".to_string(), "n2".to_string(), "n3".to_string()],
        );

        let forward_message = Message {
            src: "n2".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::ForwardSend {
                msg_id: 10,
                orig_src: "c1".to_string(),
                orig_msg_id: 42,
                key: "k1".to_string(),
                msg: 123,
            },
        };

        let responses = handler.handle(&mut node, forward_message);

        // Should send replication messages to peers
        assert_eq!(responses.len(), 2);

        // Check replication messages
        for response in responses.iter() {
            assert_eq!(response.src, "n1");
            assert!(response.dest == "n2" || response.dest == "n3");
            match &response.body {
                MessageBody::Replicate {
                    msg_id: _,
                    key,
                    msg,
                    offset,
                } => {
                    assert_eq!(key, "k1");
                    assert_eq!(*msg, 123);
                    assert_eq!(*offset, 0);
                }
                _ => panic!("Expected Replicate message"),
            }
        }

        // Should have pending operation with original client info
        assert_eq!(handler.pendings.len(), 1);
        let pending = handler.pendings.get(&0).unwrap();
        assert_eq!(pending.client, "c1");
        assert_eq!(pending.client_msg_id, 42);
        assert_eq!(pending.acks, 1);
    }

    #[test]
    fn test_handles_replicate_message() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();

        // Initialize as follower
        handler.handle_init(
            &mut node,
            "n2".to_string(),
            vec!["n1".to_string(), "n2".to_string(), "n3".to_string()],
        );

        let replicate_message = Message {
            src: "n1".to_string(),
            dest: "n2".to_string(),
            body: MessageBody::Replicate {
                msg_id: 10,
                key: "k1".to_string(),
                msg: 123,
                offset: 5,
            },
        };

        let responses = handler.handle(&mut node, replicate_message);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n2");
        assert_eq!(responses[0].dest, "n1");

        match &responses[0].body {
            MessageBody::ReplicateOk {
                msg_id: _,
                in_reply_to,
                offset,
            } => {
                assert_eq!(*in_reply_to, 10);
                assert_eq!(*offset, 5);
            }
            _ => panic!("Expected ReplicateOk message"),
        }
    }

    #[test]
    fn test_handles_replicate_ok_reaches_quorum() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();

        // Initialize as leader in 3-node cluster
        handler.handle_init(
            &mut node,
            "n1".to_string(),
            vec!["n1".to_string(), "n2".to_string(), "n3".to_string()],
        );

        // Simulate a pending operation (normally created by handle_send)
        handler.pendings.insert(
            0,
            Pending {
                client: "c1".to_string(),
                client_msg_id: 42,
                acks: 1, // Leader already counted as 1 ack
                from: HashSet::from([node.id.clone()]),
            },
        );

        // First ReplicateOk - should reach quorum (2 out of 3)
        let replicate_ok1 = Message {
            src: "n2".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::ReplicateOk {
                msg_id: 11,
                in_reply_to: 10,
                offset: 0,
            },
        };

        let responses = handler.handle(&mut node, replicate_ok1);

        // Should respond to client now that quorum is reached
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n1");
        assert_eq!(responses[0].dest, "c1");

        match &responses[0].body {
            MessageBody::SendOk {
                msg_id: _,
                in_reply_to,
                offset,
            } => {
                assert_eq!(*in_reply_to, 42);
                assert_eq!(*offset, 0);
            }
            _ => panic!("Expected SendOk message"),
        }

        // Pending operation should be removed
        assert_eq!(handler.pendings.len(), 0);
    }

    #[test]
    fn test_handles_replicate_ok_not_quorum_yet() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();

        // Initialize as leader in 5-node cluster (quorum = 3)
        handler.handle_init(
            &mut node,
            "n1".to_string(),
            vec![
                "n1".to_string(),
                "n2".to_string(),
                "n3".to_string(),
                "n4".to_string(),
                "n5".to_string(),
            ],
        );

        // Simulate a pending operation
        handler.pendings.insert(
            0,
            Pending {
                client: "c1".to_string(),
                client_msg_id: 42,
                acks: 1, // Leader already counted as 1 ack
                from: HashSet::from([node.id.clone()]),
            },
        );

        // First ReplicateOk - not enough for quorum yet
        let replicate_ok1 = Message {
            src: "n2".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::ReplicateOk {
                msg_id: 11,
                in_reply_to: 10,
                offset: 0,
            },
        };

        let responses = handler.handle(&mut node, replicate_ok1);

        // Should not respond to client yet
        assert_eq!(responses.len(), 0);

        // Pending operation should still exist with incremented acks
        assert_eq!(handler.pendings.len(), 1);
        let pending = handler.pendings.get(&0).unwrap();
        assert_eq!(pending.acks, 2);
    }

    #[test]
    fn test_handles_poll_message() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();

        // Initialize node
        handler.handle_init(&mut node, "n1".to_string(), vec!["n1".to_string()]);

        // Add some data first
        handler.logs.insert_at("k1", 0, 123);
        handler.logs.insert_at("k1", 1, 456);
        handler.logs.insert_at("k2", 0, 789);

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
            MessageBody::PollOk {
                msg_id: _,
                in_reply_to,
                msgs,
            } => {
                assert_eq!(*in_reply_to, 10);
                assert!(msgs.contains_key("k1"));
                assert!(msgs.contains_key("k2"));
            }
            _ => panic!("Expected PollOk message"),
        }
    }

    #[test]
    fn test_handles_commit_offsets_message() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();

        // Initialize node
        handler.handle_init(&mut node, "n1".to_string(), vec!["n1".to_string()]);

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
            MessageBody::CommitOffsetsOk {
                msg_id: _,
                in_reply_to,
            } => {
                assert_eq!(*in_reply_to, 42);
            }
            _ => panic!("Expected CommitOffsetsOk message"),
        }
    }

    #[test]
    fn test_handles_list_committed_offsets_message() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();

        // Initialize node
        handler.handle_init(&mut node, "n1".to_string(), vec!["n1".to_string()]);

        // Add some data first to create the logs
        handler.logs.insert_at("k1", 0, 123);
        handler.logs.insert_at("k2", 0, 456);

        // First commit some offsets
        let mut commit_offsets = HashMap::new();
        commit_offsets.insert("k1".to_string(), 100);
        commit_offsets.insert("k2".to_string(), 200);
        handler.logs.commit_offsets(commit_offsets);

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
            MessageBody::ListCommittedOffsetsOk {
                msg_id: _,
                in_reply_to,
                offsets,
            } => {
                assert_eq!(*in_reply_to, 10);
                // Check that we get the committed offsets back, or defaults
                assert!(offsets.contains_key("k1"));
                assert!(offsets.contains_key("k2"));
                // k3 might not be present since it wasn't used
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
    fn test_multi_node_workflow_with_quorum() {
        let mut leader = KafkaNode::new();
        let mut leader_node = Node::new();
        let mut follower1 = KafkaNode::new();
        let mut follower1_node = Node::new();
        let mut follower2 = KafkaNode::new();
        let mut follower2_node = Node::new();

        // Initialize all nodes
        leader.handle_init(
            &mut leader_node,
            "n1".to_string(),
            vec!["n1".to_string(), "n2".to_string(), "n3".to_string()],
        );
        follower1.handle_init(
            &mut follower1_node,
            "n2".to_string(),
            vec!["n1".to_string(), "n2".to_string(), "n3".to_string()],
        );
        follower2.handle_init(
            &mut follower2_node,
            "n3".to_string(),
            vec!["n1".to_string(), "n2".to_string(), "n3".to_string()],
        );

        // Client sends message to non-leader
        let client_send = Message {
            src: "c1".to_string(),
            dest: "n2".to_string(),
            body: MessageBody::Send {
                msg_id: 1,
                key: "test-key".to_string(),
                msg: 42,
            },
        };

        // Follower forwards to leader
        let forward_responses = follower1.handle(&mut follower1_node, client_send);
        assert_eq!(forward_responses.len(), 1);

        let forward_msg = &forward_responses[0];
        assert_eq!(forward_msg.dest, "n1"); // To leader

        // Leader handles forwarded message
        let leader_responses = leader.handle(&mut leader_node, forward_msg.clone());
        assert_eq!(leader_responses.len(), 2); // Two replication messages

        // Extract the msg_id from one of the replication messages
        let replicate_msg_id = match &leader_responses[0].body {
            MessageBody::Replicate { msg_id, .. } => *msg_id,
            _ => panic!("Expected Replicate message"),
        };

        // Simulate one follower acknowledging replication
        let replicate_ok = Message {
            src: "n2".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::ReplicateOk {
                msg_id: 100,
                in_reply_to: replicate_msg_id,
                offset: 0,
            },
        };

        let final_responses = leader.handle(&mut leader_node, replicate_ok);

        // Should get client response once quorum is reached
        assert_eq!(final_responses.len(), 1);
        assert_eq!(final_responses[0].dest, "c1");
        match &final_responses[0].body {
            MessageBody::SendOk {
                in_reply_to,
                offset,
                ..
            } => {
                assert_eq!(*in_reply_to, 1);
                assert_eq!(*offset, 0);
            }
            _ => panic!("Expected SendOk message"),
        }
    }

    #[test]
    fn test_pending_operations_cleanup() {
        let mut handler = KafkaNode::new();
        let mut node = Node::new();

        // Initialize as leader
        handler.handle_init(
            &mut node,
            "n1".to_string(),
            vec!["n1".to_string(), "n2".to_string(), "n3".to_string()],
        );

        // Send message to create pending operation
        let send_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Send {
                msg_id: 42,
                key: "k1".to_string(),
                msg: 123,
            },
        };

        handler.handle(&mut node, send_message);
        assert_eq!(handler.pendings.len(), 1);

        // Send enough ReplicateOk messages to reach quorum
        let replicate_ok = Message {
            src: "n2".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::ReplicateOk {
                msg_id: 11,
                in_reply_to: 10,
                offset: 0,
            },
        };

        handler.handle(&mut node, replicate_ok);

        // Pending operation should be cleaned up after reaching quorum
        assert_eq!(handler.pendings.len(), 0);
    }
}
