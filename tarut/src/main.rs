use maelstrom::{Message, MessageBody};
use std::collections::HashMap;
use tokio::{
    io::{self, AsyncBufReadExt, BufReader},
    sync::mpsc,
};

struct KV {
    entries: HashMap<u64, Option<u64>>,
}

impl KV {
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
}

struct Node {
    /// Unique node identifier
    id: String,
    /// Peer node IDs for gossip
    peers: Vec<String>,
    /// Message counter for generating unique msg_ids
    msg_id: u64,
    /// Key-value store to process cluster transactions
    kv: KV,
}

impl Node {
    fn new() -> Self {
        Self {
            id: String::new(),
            peers: Vec::new(),
            msg_id: 0,
            kv: KV::new(),
        }
    }

    fn handle_init(&mut self, node_id: String, node_ids: Vec<String>) {
        self.id = node_id.clone();
        self.peers = node_ids.clone();
        self.peers.retain(|p| p != &self.id);
    }

    fn handle_tx(
        &mut self,
        message: Message,
        msg_id: u64,
        txn: Vec<(String, u64, Option<u64>)>,
    ) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        // Apply read+write ops locally
        let results = self.kv.process_txn(txn.clone());
        // Broadcast *only* writes to each peer
        let write_ops: Vec<_> = txn.into_iter().filter(|(op, _, _)| op == "w").collect();

        for peer in &self.peers {
            self.msg_id += 1;
            out.push(Message {
                src: self.id.clone(),
                dest: peer.clone(),
                body: MessageBody::TarutReplicate {
                    msg_id: self.msg_id,
                    txn: write_ops.clone(),
                },
            })
        }

        // reply to client immediately
        out.push(Message {
            src: self.id.clone(),
            dest: message.src,
            body: MessageBody::TxnOk {
                msg_id: self.msg_id,
                in_reply_to: msg_id,
                txn: results,
            },
        });

        out
    }

    fn process_message(&mut self, message: Message) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        self.msg_id += 1;
        match message.body.clone() {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                self.handle_init(node_id, node_ids);
                out.push(Message {
                    src: self.id.clone(),
                    dest: message.src,
                    body: MessageBody::InitOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                    },
                })
            }
            MessageBody::Txn { msg_id, txn } => {
                let messages = self.handle_tx(message, msg_id, txn);
                out.extend(messages);
            }
            MessageBody::TarutReplicate { txn, .. } => {
                // idempotently apply peer-originated writes,
                // but do not rebroadcast them
                self.kv.process_txn(txn);
            }
            _ => {}
        }
        out
    }
}

#[tokio::main]
async fn main() {
    let mut node = Node::new();
    let (tx, mut rx) = mpsc::channel::<Message>(32);

    // Spawn stdin reader
    let stdin_tx = tx.clone();
    tokio::spawn(async move {
        let reader = BufReader::new(io::stdin());
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Ok(msg) = serde_json::from_str::<Message>(&line) {
                let _ = stdin_tx.send(msg).await;
            }
        }
    });

    while let Some(msg) = rx.recv().await {
        for response in node.process_message(msg) {
            let response_str = serde_json::to_string(&response).unwrap();
            println!("{response_str}");
        }
    }
}
