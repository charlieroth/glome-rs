use maelstrom::{
    Message, MessageBody,
    node::{MessageHandler, Node},
};
use std::collections::HashMap;

pub struct TatNode {
    /// Key-value store to process cluster transactions
    entries: HashMap<u64, Option<u64>>,
}

impl Default for TatNode {
    fn default() -> Self {
        Self::new()
    }
}

impl TatNode {
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
                _ => unreachable!("unknown transaction operation"),
            }
        }
        results
    }
}

impl MessageHandler for TatNode {
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
                let results = self.process_txn(txn);
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    message.src,
                    MessageBody::TxnOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                        txn: results,
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

    #[test]
    fn test_tat_node_new() {
        let node = TatNode::new();
        assert!(node.entries.is_empty());
    }

    #[test]
    fn test_tat_node_default() {
        let node = TatNode::default();
        assert!(node.entries.is_empty());
    }

    #[test]
    fn test_process_txn_read_nonexistent_key() {
        let mut node = TatNode::new();
        let txn = vec![("r".to_string(), 1, None)];
        let results = node.process_txn(txn);
        
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], ("r".to_string(), 1, None));
    }

    #[test]
    fn test_process_txn_write_operation() {
        let mut node = TatNode::new();
        let txn = vec![("w".to_string(), 1, Some(42))];
        let results = node.process_txn(txn);
        
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], ("w".to_string(), 1, Some(42)));
        assert_eq!(node.entries.get(&1), Some(&Some(42)));
    }

    #[test]
    fn test_process_txn_write_then_read() {
        let mut node = TatNode::new();
        let txn = vec![
            ("w".to_string(), 1, Some(42)),
            ("r".to_string(), 1, None),
        ];
        let results = node.process_txn(txn);
        
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], ("w".to_string(), 1, Some(42)));
        assert_eq!(results[1], ("r".to_string(), 1, Some(42)));
    }

    #[test]
    fn test_process_txn_write_null_value() {
        let mut node = TatNode::new();
        let txn = vec![("w".to_string(), 1, None)];
        let results = node.process_txn(txn);
        
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], ("w".to_string(), 1, None));
        assert_eq!(node.entries.get(&1), Some(&None));
    }

    #[test]
    fn test_process_txn_overwrite_value() {
        let mut node = TatNode::new();
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
    }

    #[test]
    fn test_process_txn_multiple_keys() {
        let mut node = TatNode::new();
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
    fn test_handle_init_message() {
        let mut handler = TatNode::new();
        let mut node = Node::new();
        
        let init_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Init {
                msg_id: 1,
                node_id: "n1".to_string(),
                node_ids: vec!["n1".to_string(), "n2".to_string()],
            },
        };

        let responses = handler.handle(&mut node, init_message);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n1");
        assert_eq!(responses[0].dest, "c1");
        if let MessageBody::InitOk { in_reply_to, .. } = &responses[0].body {
            assert_eq!(*in_reply_to, 1);
        } else {
            panic!("Expected InitOk message body");
        }
    }

    #[test]
    fn test_handle_txn_message() {
        let mut handler = TatNode::new();
        let mut node = Node::new();
        
        // Initialize the node first
        node.handle_init("n1".to_string(), vec!["n1".to_string(), "n2".to_string()]);
        
        let txn_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Txn {
                msg_id: 1,
                txn: vec![
                    ("w".to_string(), 1, Some(42)),
                    ("r".to_string(), 1, None),
                ],
            },
        };

        let responses = handler.handle(&mut node, txn_message);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n1");
        assert_eq!(responses[0].dest, "c1");
        if let MessageBody::TxnOk { in_reply_to, txn, .. } = &responses[0].body {
            assert_eq!(*in_reply_to, 1);
            assert_eq!(txn.len(), 2);
            assert_eq!(txn[0], ("w".to_string(), 1, Some(42)));
            assert_eq!(txn[1], ("r".to_string(), 1, Some(42)));
        } else {
            panic!("Expected TxnOk message body");
        }
    }

    #[test]
    fn test_handle_unknown_message() {
        let mut handler = TatNode::new();
        let mut node = Node::new();
        
        let echo_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Echo {
                msg_id: 1,
                echo: "test".to_string(),
            },
        };

        let responses = handler.handle(&mut node, echo_message);
        assert_eq!(responses.len(), 0);
    }
}
