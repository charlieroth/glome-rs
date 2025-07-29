use maelstrom::{ErrorCode, Message, MessageBody, MessageHandler, Node};
use std::collections::HashMap;

pub struct KV {
    /// Committed values: key -> optional value
    entries: HashMap<u64, Option<u64>>,
    /// Version (commit timestamp) of the last write for each key
    versions: HashMap<u64, u64>,
}

impl Default for KV {
    fn default() -> Self {
        Self::new()
    }
}

impl KV {
    pub fn new() -> Self {
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

pub struct TarctNode {
    /// Committed key-value store with version tracking
    kv: KV,
    /// Logical clock for local commits
    commit_ts: u64,
}

impl TarctNode {
    pub fn new() -> Self {
        Self {
            kv: KV::new(),
            commit_ts: 0,
        }
    }

    fn handle_tx(
        &mut self,
        node: &mut Node,
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
                // abort on conflict
                out.push(Message {
                    src: node.id.clone(),
                    dest: message.src.clone(),
                    body: MessageBody::Error {
                        msg_id: node.next_msg_id(),
                        in_reply_to: msg_id,
                        code: ErrorCode::TxnConflict,
                        text: Some("Transaction aborted. Conflict detected".into()),
                        extra: None,
                    },
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

        let peers = node.peers.clone();
        for peer in &peers {
            out.push(Message {
                src: node.id.clone(),
                dest: peer.clone(),
                body: MessageBody::TarctReplicate {
                    msg_id: node.next_msg_id(),
                    txn: replicate_ops.clone(),
                },
            })
        }

        // reply to client
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

impl MessageHandler for TarctNode {
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
            MessageBody::Txn { msg_id, txn } => {
                let messages = self.handle_tx(node, message, msg_id, txn);
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
