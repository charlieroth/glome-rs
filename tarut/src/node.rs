use maelstrom::{
    Message, MessageBody,
    node::{MessageHandler, Node},
};
use std::collections::HashMap;

pub struct TarutNode {
    /// Key-value store to process cluster transactions
    entries: HashMap<u64, Option<u64>>,
}

impl Default for TarutNode {
    fn default() -> Self {
        Self::new()
    }
}

impl TarutNode {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn process_txn(
        &mut self,
        txn: Vec<(String, u64, Option<u64>)>,
    ) -> Vec<(String, u64, Option<u64>)> {
        let mut results = Vec::with_capacity(txn.len());
        for (op, key, opt_val) in txn {
            match op.as_str() {
                "r" => {
                    let read_val = self.entries.get(&key).and_then(|v| *v);
                    results.push(("r".to_string(), key, read_val));
                }
                "w" => {
                    self.entries.insert(key, opt_val);
                    results.push(("w".to_string(), key, opt_val));
                }
                _ => {}
            }
        }
        results
    }

    pub fn handle_tx(
        &mut self,
        node: &mut Node,
        message: Message,
        msg_id: u64,
        txn: Vec<(String, u64, Option<u64>)>,
    ) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        // Apply read+write ops locally
        let results = self.process_txn(txn.clone());
        // Broadcast *only* writes to each peer
        let write_ops: Vec<_> = txn.into_iter().filter(|(op, _, _)| op == "w").collect();

        let peers = node.peers.clone();
        for peer in &peers {
            out.push(Message {
                src: node.id.clone(),
                dest: peer.clone(),
                body: MessageBody::TarutReplicate {
                    msg_id: node.next_msg_id(),
                    txn: write_ops.clone(),
                },
            })
        }

        // reply to client immediately
        out.push(Message {
            src: node.id.clone(),
            dest: message.src,
            body: MessageBody::TxnOk {
                msg_id: node.next_msg_id(),
                in_reply_to: msg_id,
                txn: results,
            },
        });

        out
    }
}

impl MessageHandler for TarutNode {
    fn handle(&mut self, node: &mut Node, message: Message) -> Vec<Message> {
        let mut out = Vec::new();
        match message.body.clone() {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                node.handle_init(node_id, node_ids);
                out.push(node.init_ok(message.src, msg_id));
            }
            MessageBody::Txn { msg_id, txn } => {
                let messages = self.handle_tx(node, message, msg_id, txn);
                out.extend(messages);
            }
            MessageBody::TarutReplicate { txn, .. } => {
                // idempotently apply peer-originated writes,
                // but do not rebroadcast them
                self.process_txn(txn);
            }
            _ => {}
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use maelstrom::ErrorCode;

    use super::*;

    #[test]
    fn test_tarut_node_new() {
        let node = TarutNode::new();
        assert!(node.entries.is_empty());
    }

    #[test]
    fn test_tarut_node_default() {
        let node = TarutNode::default();
        assert!(node.entries.is_empty());
    }

    #[test]
    fn test_process_txn_read_nonexistent_key() {
        let mut node = TarutNode::new();
        let txn = vec![("r".to_string(), 1, None)];
        let results = node.process_txn(txn);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0], ("r".to_string(), 1, None));
    }

    #[test]
    fn test_process_txn_write_operation() {
        let mut node = TarutNode::new();
        let txn = vec![("w".to_string(), 1, Some(42))];
        let results = node.process_txn(txn);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0], ("w".to_string(), 1, Some(42)));
        assert_eq!(node.entries.get(&1), Some(&Some(42)));
    }

    #[test]
    fn test_process_txn_write_then_read() {
        let mut node = TarutNode::new();
        let txn = vec![("w".to_string(), 1, Some(42)), ("r".to_string(), 1, None)];
        let results = node.process_txn(txn);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0], ("w".to_string(), 1, Some(42)));
        assert_eq!(results[1], ("r".to_string(), 1, Some(42)));
    }

    #[test]
    fn test_process_txn_write_null_value() {
        let mut node = TarutNode::new();
        let txn = vec![("w".to_string(), 1, None)];
        let results = node.process_txn(txn);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0], ("w".to_string(), 1, None));
        assert_eq!(node.entries.get(&1), Some(&None));
    }

    #[test]
    fn test_process_txn_overwrite_value() {
        let mut node = TarutNode::new();
        let txn = vec![
            ("w".to_string(), 1, Some(42)),
            ("w".to_string(), 1, Some(99)),
            ("r".to_string(), 1, None),
        ];
        let results = node.process_txn(txn);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0], ("w".to_string(), 1, Some(42)));
        assert_eq!(results[1], ("w".to_string(), 1, Some(99)));
        assert_eq!(results[2], ("r".to_string(), 1, Some(99)));
        assert_eq!(node.entries.get(&1), Some(&Some(99)));
    }

    #[test]
    fn test_process_txn_multiple_keys() {
        let mut node = TarutNode::new();
        let txn = vec![
            ("w".to_string(), 1, Some(10)),
            ("w".to_string(), 2, Some(20)),
            ("r".to_string(), 1, None),
            ("r".to_string(), 2, None),
            ("r".to_string(), 3, None),
        ];
        let results = node.process_txn(txn);

        assert_eq!(results.len(), 5);
        assert_eq!(results[0], ("w".to_string(), 1, Some(10)));
        assert_eq!(results[1], ("w".to_string(), 2, Some(20)));
        assert_eq!(results[2], ("r".to_string(), 1, Some(10)));
        assert_eq!(results[3], ("r".to_string(), 2, Some(20)));
        assert_eq!(results[4], ("r".to_string(), 3, None));
    }

    #[test]
    fn test_process_txn_unknown_operation() {
        let mut node = TarutNode::new();
        let txn = vec![
            ("w".to_string(), 1, Some(42)),
            ("unknown".to_string(), 2, Some(99)),
            ("r".to_string(), 1, None),
        ];
        let results = node.process_txn(txn);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0], ("w".to_string(), 1, Some(42)));
        assert_eq!(results[1], ("r".to_string(), 1, Some(42)));
    }

    #[test]
    fn test_handle_tx_creates_correct_messages() {
        let mut tarut_node = TarutNode::new();
        let mut node = Node::new();
        node.handle_init(
            "node1".to_string(),
            vec![
                "node1".to_string(),
                "node2".to_string(),
                "node3".to_string(),
            ],
        );

        let message = Message {
            src: "client".to_string(),
            dest: "node1".to_string(),
            body: MessageBody::Txn {
                msg_id: 1,
                txn: vec![
                    ("w".to_string(), 1, Some(42)),
                    ("r".to_string(), 1, None),
                    ("w".to_string(), 2, Some(99)),
                ],
            },
        };

        let txn = vec![
            ("w".to_string(), 1, Some(42)),
            ("r".to_string(), 1, None),
            ("w".to_string(), 2, Some(99)),
        ];

        let out_messages = tarut_node.handle_tx(&mut node, message, 1, txn);

        // Should generate: 2 replicate messages (to peers) + 1 TxnOk message (to client)
        assert_eq!(out_messages.len(), 3);

        // Check replicate messages to peers
        let replicate_msgs: Vec<_> = out_messages
            .iter()
            .filter(|msg| matches!(msg.body, MessageBody::TarutReplicate { .. }))
            .collect();
        assert_eq!(replicate_msgs.len(), 2);

        // Check TxnOk message to client
        let txn_ok_msgs: Vec<_> = out_messages
            .iter()
            .filter(|msg| matches!(msg.body, MessageBody::TxnOk { .. }))
            .collect();
        assert_eq!(txn_ok_msgs.len(), 1);

        if let MessageBody::TxnOk {
            in_reply_to, txn, ..
        } = &txn_ok_msgs[0].body
        {
            assert_eq!(*in_reply_to, 1);
            assert_eq!(txn.len(), 3);
            assert_eq!(txn[0], ("w".to_string(), 1, Some(42)));
            assert_eq!(txn[1], ("r".to_string(), 1, Some(42)));
            assert_eq!(txn[2], ("w".to_string(), 2, Some(99)));
        } else {
            panic!("Expected TxnOk message");
        }
    }

    #[test]
    fn test_handle_tx_only_replicates_writes() {
        let mut tarut_node = TarutNode::new();
        let mut node = Node::new();
        node.handle_init(
            "node1".to_string(),
            vec!["node1".to_string(), "node2".to_string()],
        );

        let message = Message {
            src: "client".to_string(),
            dest: "node1".to_string(),
            body: MessageBody::Txn {
                msg_id: 1,
                txn: vec![],
            },
        };

        let txn = vec![
            ("r".to_string(), 1, None),
            ("w".to_string(), 2, Some(99)),
            ("r".to_string(), 3, None),
        ];

        let out_messages = tarut_node.handle_tx(&mut node, message, 1, txn);

        // Check that only write operations are replicated
        let replicate_msgs: Vec<_> = out_messages
            .iter()
            .filter(|msg| matches!(msg.body, MessageBody::TarutReplicate { .. }))
            .collect();

        if let MessageBody::TarutReplicate { txn, .. } = &replicate_msgs[0].body {
            assert_eq!(txn.len(), 1);
            assert_eq!(txn[0], ("w".to_string(), 2, Some(99)));
        } else {
            panic!("Expected TarutReplicate message");
        }
    }

    #[test]
    fn test_message_handler_init() {
        let mut tarut_node = TarutNode::new();
        let mut node = Node::new();

        let message = Message {
            src: "maelstrom".to_string(),
            dest: "node1".to_string(),
            body: MessageBody::Init {
                msg_id: 1,
                node_id: "node1".to_string(),
                node_ids: vec!["node1".to_string(), "node2".to_string()],
            },
        };

        let out_messages = tarut_node.handle(&mut node, message);

        assert_eq!(out_messages.len(), 1);
        assert_eq!(node.id, "node1");
        assert_eq!(node.peers, vec!["node2"]);

        if let MessageBody::InitOk { in_reply_to, .. } = &out_messages[0].body {
            assert_eq!(*in_reply_to, 1);
        } else {
            panic!("Expected InitOk message");
        }
    }

    #[test]
    fn test_message_handler_txn() {
        let mut tarut_node = TarutNode::new();
        let mut node = Node::new();
        node.handle_init(
            "node1".to_string(),
            vec!["node1".to_string(), "node2".to_string()],
        );

        let message = Message {
            src: "client".to_string(),
            dest: "node1".to_string(),
            body: MessageBody::Txn {
                msg_id: 1,
                txn: vec![("w".to_string(), 1, Some(42))],
            },
        };

        let out_messages = tarut_node.handle(&mut node, message);

        // Should generate replicate + TxnOk messages
        assert!(out_messages.len() >= 2);

        let txn_ok_msgs: Vec<_> = out_messages
            .iter()
            .filter(|msg| matches!(msg.body, MessageBody::TxnOk { .. }))
            .collect();
        assert_eq!(txn_ok_msgs.len(), 1);
    }

    #[test]
    fn test_message_handler_tarut_replicate() {
        let mut tarut_node = TarutNode::new();
        let mut node = Node::new();

        let message = Message {
            src: "node2".to_string(),
            dest: "node1".to_string(),
            body: MessageBody::TarutReplicate {
                msg_id: 1,
                txn: vec![("w".to_string(), 1, Some(42))],
            },
        };

        let out_messages = tarut_node.handle(&mut node, message);

        // Should not generate any outgoing messages for replication
        assert_eq!(out_messages.len(), 0);

        // But should apply the transaction locally
        assert_eq!(tarut_node.entries.get(&1), Some(&Some(42)));
    }

    #[test]
    fn test_message_handler_unknown_message() {
        let mut tarut_node = TarutNode::new();
        let mut node = Node::new();

        let message = Message {
            src: "unknown".to_string(),
            dest: "node1".to_string(),
            body: MessageBody::Error {
                msg_id: 1,
                in_reply_to: 1,
                code: ErrorCode::Other,
                text: Some("unknown".to_string()),
                extra: None,
            },
        };

        let out_messages = tarut_node.handle(&mut node, message);

        // Should not generate any messages for unknown message types
        assert_eq!(out_messages.len(), 0);
    }

    #[test]
    fn test_read_uncommitted_consistency() {
        let mut tarut_node = TarutNode::new();

        // Simulate concurrent transactions that could cause dirty reads
        let txn1 = vec![("w".to_string(), 1, Some(100))];
        let txn2 = vec![("r".to_string(), 1, None)];

        // Apply write first
        let results1 = tarut_node.process_txn(txn1);
        assert_eq!(results1[0], ("w".to_string(), 1, Some(100)));

        // Read should see uncommitted write (read uncommitted behavior)
        let results2 = tarut_node.process_txn(txn2);
        assert_eq!(results2[0], ("r".to_string(), 1, Some(100)));
    }

    #[test]
    fn test_dirty_write_prevention() {
        let mut tarut_node = TarutNode::new();

        // Test that writes properly overwrite previous values
        let txn = vec![
            ("w".to_string(), 1, Some(1)),
            ("w".to_string(), 1, Some(2)),
            ("w".to_string(), 1, Some(3)),
            ("r".to_string(), 1, None),
        ];

        let results = tarut_node.process_txn(txn);

        // Should see the final write value
        assert_eq!(results[3], ("r".to_string(), 1, Some(3)));
        assert_eq!(tarut_node.entries.get(&1), Some(&Some(3)));
    }
}
