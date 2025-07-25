use maelstrom::{ErrorCode, Message, MessageBody};
use std::{
    collections::HashMap,
    io::{self, BufRead, BufReader},
};
use tokio::sync::mpsc;

struct KV {
    /// Committed values: key -> optional value
    entries: HashMap<u64, Option<u64>>,
    /// Version (commit timestamp) of the last write for each key
    versions: HashMap<u64, u64>,
}

impl KV {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            versions: HashMap::new(),
        }
    }

    /// Retrieves the committed value for a given key
    /// Returns `None` if th ekey is not present or has been deleted
    pub fn get(&self, key: &u64) -> Option<u64> {
        self.entries.get(key).cloned().unwrap_or(None)
    }

    /// Retrieves the commit timestamp (version) for a given key
    /// Returns 0 if the key is not present
    pub fn version(&self, key: &u64) -> u64 {
        *self.versions.get(key).unwrap_or(&0)
    }

    /// Applies a committed write to the store
    pub fn apply(&mut self, key: u64, val: Option<u64>, version: u64) {
        let current_version = self.version(&key);
        if version > current_version {
            self.entries.insert(key, val);
            self.versions.insert(key, version);
        }
    }

    pub fn merge_batch(&mut self, writes: Vec<(u64, Option<u64>, u64)>) {
        for (key, val, version) in writes {
            self.apply(key, val, version)
        }
    }
}

struct Node {
    /// Unique node identifier
    id: String,
    /// Peer node IDs for gossip
    peers: Vec<String>,
    /// Message counter for generating unique msg_ids
    msg_id: u64,
    /// Committed key-value store with version tracking
    kv: KV,
    /// Logical clock for local commits
    commit_ts: u64,
}

impl Node {
    fn new() -> Self {
        Self {
            id: String::new(),
            peers: Vec::new(),
            msg_id: 0,
            kv: KV::new(),
            commit_ts: 0,
        }
    }

    async fn handle_init(&mut self, node_id: String, node_ids: Vec<String>) {
        self.id = node_id.clone();
        self.peers = node_ids.clone();
        self.peers.retain(|p| p != &self.id);
    }

    async fn handle_tx(
        &mut self,
        message: Message,
        msg_id: u64,
        txn: Vec<(String, u64, Option<u64>)>,
    ) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();

        // stage read-set and write-set
        let mut read_set: HashMap<u64, u64> = HashMap::new();
        let mut write_set: HashMap<u64, Option<u64>> = HashMap::new();
        let mut results = Vec::with_capacity(txn.len());

        // execute operations against staging area
        for (op, key, opt_val) in txn.iter() {
            match op.as_str() {
                "r" => {
                    // check uncommitted writes first, then committed store
                    let val = write_set
                        .get(key)
                        .cloned()
                        .unwrap_or_else(|| self.kv.get(key));
                    // record observed version
                    let version = self.kv.version(key);
                    read_set.insert(*key, version);
                    results.push(("r".to_string(), *key, val));
                }
                "w" => {
                    write_set.insert(*key, *opt_val);
                    results.push(("w".to_string(), *key, *opt_val));
                }
                _ => unreachable!("Unknown operation"),
            }
        }

        // optimistic conflict check against current committed versions
        for (&key, &seen_version) in read_set.iter() {
            let current_version = self.kv.version(&key);
            if current_version != seen_version {
                self.msg_id += 1;
                // abort on conflict
                out.push(Message{
                    src: self.id.clone(),
                    dest: message.src.clone(),
                    body: MessageBody::Error {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                        code: ErrorCode::TxnConflict,
                        text: Some("The requested transaction has been aborted because of a conflict with another transaction. Server need not return this error on every conflict: they may choose to retry automatically instead.".into()),
                        extra: None
                    }
                })
            }
        }

        // commit: bump local clock and install writes
        self.commit_ts += 1;
        let this_ts = self.commit_ts;
        for (&key, &val) in write_set.iter() {
            self.kv.apply(key, val, this_ts);
        }

        // gossip the committed writes (including version) to all peers
        // prepare batch: ("w", key, val, version)
        let replicate_ops: Vec<(String, u64, Option<u64>, u64)> = write_set
            .iter()
            .map(|(&key, &val)| ("w".to_string(), key, val, this_ts))
            .collect();

        for peer in &self.peers {
            self.msg_id += 1;
            out.push(Message {
                src: self.id.clone(),
                dest: peer.clone(),
                body: MessageBody::TarctReplicate {
                    msg_id: self.msg_id,
                    txn: replicate_ops.clone(),
                },
            })
        }

        // reply to client
        self.msg_id += 1;
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

    async fn process_message(&mut self, message: Message) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        self.msg_id += 1;
        match message.body.clone() {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                self.handle_init(node_id, node_ids).await;
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
                let messages = self.handle_tx(message, msg_id, txn).await;
                out.extend(messages);
            }
            MessageBody::TarctReplicate {
                msg_id: _,
                txn: batch,
            } => {
                let writes = batch
                    .iter()
                    .filter(|(op, _, _, _)| op == "w")
                    .map(|(_, key, val, version)| (*key, *val, *version))
                    .collect();
                self.kv.merge_batch(writes);
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
        let responses = node.process_message(msg).await;
        for response in responses {
            let response_str = serde_json::to_string(&response).unwrap();
            println!("{response_str}");
        }
    }
}
