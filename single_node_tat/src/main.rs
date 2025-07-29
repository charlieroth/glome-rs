use maelstrom::{
    Message, MessageBody,
    node::{MessageHandler, Node, run_node},
};
use std::collections::HashMap;

struct TatNode {
    /// Key-value store to process cluster transactions
    entries: HashMap<u64, Option<u64>>,
}

impl TatNode {
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

#[tokio::main]
async fn main() {
    let handler = TatNode::new();
    run_node(handler).await;
}
