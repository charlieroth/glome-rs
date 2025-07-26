use maelstrom::{Message, MessageBody};
use std::{
    collections::HashMap,
    io::{self, BufRead, BufReader},
};
use tokio::sync::mpsc;

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
        txns: Vec<(String, u64, Option<u64>)>,
    ) -> Vec<(String, u64, Option<u64>)> {
        let mut results: Vec<(String, u64, Option<u64>)> = Vec::new();
        for txn in txns {
            if txn.0 == "r" {
                if let Some(value) = self.get(&txn.1) {
                    results.push(("r".to_string(), txn.1, Some(value)));
                } else {
                    results.push(("r".to_string(), txn.1, None));
                }
            } else if txn.0 == "w" {
                self.put(txn.1, txn.2);
                results.push(("w".to_string(), txn.1, txn.2));
            } else {
                eprintln!("unknown transaction type: {txn:?}");
            }
        }

        results
    }

    fn put(&mut self, key: u64, value: Option<u64>) {
        self.entries.insert(key, value);
    }

    fn get(&self, key: &u64) -> Option<u64> {
        *self.entries.get(key).unwrap_or(&None)
    }
}

struct Node {
    id: String,
    peers: Vec<String>,
    msg_id: u64,
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

    async fn handle_init(&mut self, node_id: String, node_ids: Vec<String>) {
        self.id = node_id.clone();
        self.peers = node_ids.clone();
        self.peers.retain(|p| p != &self.id);
    }

    async fn process_message(&mut self, message: Message) -> Option<Message> {
        self.msg_id += 1;
        match message.body {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                self.handle_init(node_id, node_ids).await;
                Some(Message {
                    src: self.id.clone(),
                    dest: message.src,
                    body: MessageBody::InitOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                    },
                })
            }
            MessageBody::Txn { msg_id, txn } => {
                let results = self.kv.process_txn(txn);
                Some(Message {
                    src: self.id.clone(),
                    dest: message.src,
                    body: MessageBody::TxnOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                        txn: results,
                    },
                })
            }
            _ => None,
        }
    }
}

#[tokio::main]
async fn main() {
    let mut node = Node::new();
    let (tx, mut rx) = mpsc::channel::<Message>(32);

    // Spawn stdin reader
    let stdin_tx = tx.clone();
    tokio::spawn(async move {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin);
        for line in reader.lines() {
            if let Ok(line) = line {
                if let Ok(msg) = serde_json::from_str::<Message>(&line) {
                    let _ = stdin_tx.send(msg).await;
                }
            }
        }
    });

    while let Some(msg) = rx.recv().await {
        if let Some(response) = node.process_message(msg).await {
            let response_str = serde_json::to_string(&response).unwrap();
            println!("{response_str}");
        }
    }
}
