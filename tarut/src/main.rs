use maelstrom::{
    Message, MessageBody,
    node::{MessageHandler, Node, run_node},
};
use std::collections::HashMap;

struct TarutNode {
    /// Key-value store to process cluster transactions
    entries: HashMap<u64, Option<u64>>,
}

impl TarutNode {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    fn process_txn(
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

    fn handle_tx(
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

#[tokio::main]
async fn main() {
    let handler = TarutNode::new();
    run_node(handler).await;
}
