use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub offset: u64,
    pub msg: u64,
}

pub struct Log {
    pub log: HashMap<String, Vec<LogEntry>>,
    pub commits: HashMap<String, u64>,
    pub offsets: HashMap<String, u64>,
}

impl Log {
    pub fn new() -> Self {
        Self {
            log: HashMap::new(),
            commits: HashMap::new(),
            offsets: HashMap::new(),
        }
    }

    pub fn append(&mut self, key: String, entry: LogEntry) {
        self.log
            .entry(key)
            .and_modify(|entries| entries.push(entry.clone()))
            .or_insert(vec![entry]);
    }

    pub fn logs(
        &mut self,
        keys_and_offsets: HashMap<String, u64>,
    ) -> HashMap<String, Vec<Vec<u64>>> {
        let mut offsets: HashMap<String, Vec<Vec<u64>>> = HashMap::new();
        for (key, offset) in keys_and_offsets.iter() {
            if !self.log.contains_key(key) {
                continue;
            }

            offsets.insert(key.into(), Vec::new());
            for entry in self.log.get(key.into()).unwrap() {
                if entry.offset >= *offset {
                    offsets
                        .entry(key.into())
                        .and_modify(|entries| entries.push(vec![entry.offset, entry.msg]))
                        .or_insert(vec![vec![entry.offset, entry.msg]]);
                }
            }
        }
        offsets
    }

    pub fn set_commit_offsets(&mut self, keys_and_offsets: HashMap<String, u64>) {
        // iterate over (k, v) pairs; duplicate keys replace the old value
        self.commits.extend(keys_and_offsets);
    }

    pub fn inc_offset(&mut self, key: String) -> u64 {
        self.offsets
            .entry(key.clone())
            .and_modify(|e| *e += 1)
            .or_insert(1);

        *self.offsets.get(&key).unwrap_or(&1)
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Counter {
    pub version: u64,
    pub value: u64,
}

/// KV store with a "Version Vector" as the storage layer
///
/// Sources:
///
/// * https://martinfowler.com/articles/patterns-of-distributed-systems/version-vector.html
/// * https://en.wikipedia.org/wiki/Version_vector
/// * https://github.com/elh/gossip-glomers/blob/main/src/4_grow_only_counter.clj
pub struct KV {
    pub counters: HashMap<String, Counter>,
}

impl KV {
    pub fn new() -> Self {
        Self {
            counters: HashMap::new(),
        }
    }

    pub fn init(&mut self, node_ids: Vec<String>) {
        self.counters = HashMap::new();
        for node_id in node_ids {
            self.counters.insert(node_id, Counter::default());
        }
    }

    pub fn add(&mut self, node_id: String, delta: u64) {
        self.counters
            .entry(node_id)
            .and_modify(|counter| {
                counter.value += delta;
                counter.version += 1;
            })
            .or_insert_with(|| Counter {
                version: 1,
                value: delta,
            });
    }

    pub fn read(&self) -> u64 {
        let mut sum = 0;
        for (_, counter) in self.counters.iter() {
            sum += counter.value;
        }
        sum
    }

    pub fn merge(&mut self, incoming: HashMap<String, Counter>) {
        for (node_id, incoming_counter) in incoming {
            let current_counter = self.counters.get(&node_id);
            match current_counter {
                Some(c) => {
                    if incoming_counter.version > c.version {
                        self.counters.entry(node_id).and_modify(|counter| {
                            counter.version = incoming_counter.version;
                            counter.value = incoming_counter.value;
                        });
                    }
                }
                None => {
                    self.counters.insert(node_id, incoming_counter);
                }
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.counters.is_empty()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Envelope<T = Body> {
    pub src: String,
    pub dest: String,
    pub body: T,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Body {
    /// Initialize request sent once to every node
    #[serde(rename = "init")]
    Init(Init),

    /// Mandatory reply to `init`
    #[serde(rename = "init_ok")]
    InitOk(InitOk),

    /// Echo request
    #[serde(rename = "echo")]
    Echo(Echo),

    /// Echo reply
    #[serde(rename = "echo_ok")]
    EchoOk(EchoOk),

    /// Generate request
    #[serde(rename = "generate")]
    Generate(Generate),

    /// Generate reply
    #[serde(rename = "generate_ok")]
    GenerateOk(GenerateOk),

    /// Broadcast request
    #[serde(rename = "broadcast")]
    Broadcast(Broadcast),

    /// Broadcast reply
    #[serde(rename = "broadcast_ok")]
    BroadcastOk(BroadcastOk),

    /// BroadcastGossip request
    #[serde(rename = "broadcast_gossip")]
    BroadcastGossip(BroadcastGossip),

    /// Read request (broadcast and counter)
    #[serde(rename = "read")]
    Read(Read),

    /// Read reply (broadcast or counter)
    #[serde(rename = "read_ok")]
    ReadOk(ReadOk),

    /// Topology request
    #[serde(rename = "topology")]
    Topology(Topology),

    /// Topology reply
    #[serde(rename = "topology_ok")]
    TopologyOk(TopologyOk),

    /// Add request
    #[serde(rename = "add")]
    Add(Add),

    /// Add reply
    #[serde(rename = "add_ok")]
    AddOk(AddOk),

    /// CounterGossip request
    #[serde(rename = "counter_gossip")]
    CounterGossip(CounterGossip),

    /// Send request
    #[serde(rename = "send")]
    Send(Send),

    /// Send reply
    #[serde(rename = "send_ok")]
    SendOk(SendOk),

    /// Poll request
    #[serde(rename = "poll")]
    Poll(Poll),

    /// Poll reply
    #[serde(rename = "poll_ok")]
    PollOk(PollOk),

    /// CommitOffsets request
    #[serde(rename = "commit_offsets")]
    CommitOffsets(CommitOffsets),

    /// CommitOffsets reply
    #[serde(rename = "commit_offsets_ok")]
    CommitOffsetsOk(CommitOffsetsOk),

    /// ListCommittedOffsets request
    #[serde(rename = "list_committed_offsets")]
    ListCommittedOffsets(ListCommittedOffsets),

    /// ListCommittedOffsets reply
    #[serde(rename = "list_committed_offsets_ok")]
    ListCommittedOffsetsOk(ListCommittedOffsetsOk),

    /// Standard error reply (definite or indefinite)
    #[serde(rename = "error")]
    Error(ErrorBody),

    /// All other message-client workload RPCs, internal
    /// gossip, ad-hoc extensions land here unchanged.
    #[serde(other)]
    Unknown,
}

/// Body of initilization message
#[derive(Debug, Serialize, Deserialize)]
pub struct Init {
    pub msg_id: u64,
    pub node_id: String,
    pub node_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitOk {
    pub msg_id: u64,
    pub in_reply_to: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Echo {
    pub msg_id: u64,
    pub echo: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EchoOk {
    pub msg_id: u64,
    pub in_reply_to: u64,
    pub echo: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Generate {
    pub msg_id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateOk {
    pub msg_id: u64,
    pub in_reply_to: u64,
    pub id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Broadcast {
    pub msg_id: u64,
    pub message: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BroadcastOk {
    pub msg_id: u64,
    pub in_reply_to: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BroadcastGossip {
    pub msg_id: u64,
    pub messages: Vec<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Add {
    pub msg_id: u64,
    pub delta: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddOk {
    pub msg_id: u64,
    pub in_reply_to: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CounterGossip {
    pub msg_id: u64,
    pub counters: HashMap<String, Counter>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Read {
    pub msg_id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadOk {
    pub msg_id: u64,
    pub in_reply_to: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Topology {
    pub msg_id: u64,
    pub topology: HashMap<String, Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TopologyOk {
    pub msg_id: u64,
    pub in_reply_to: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Send {
    pub msg_id: u64,
    pub key: String,
    pub msg: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendOk {
    pub msg_id: u64,
    pub in_reply_to: u64,
    pub offset: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Poll {
    pub msg_id: u64,
    pub offsets: HashMap<String, u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PollOk {
    pub msg_id: u64,
    pub in_reply_to: u64,
    pub msgs: HashMap<String, Vec<Vec<u64>>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitOffsets {
    pub msg_id: u64,
    pub offsets: HashMap<String, u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitOffsetsOk {
    pub msg_id: u64,
    pub in_reply_to: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListCommittedOffsets {
    pub msg_id: u64,
    pub keys: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListCommittedOffsetsOk {
    pub msg_id: u64,
    pub in_reply_to: u64,
    pub offsets: HashMap<String, u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorBody {
    pub in_reply_to: u64,
    pub code: ErrorCode,
    /// Optional human-readable description
    pub text: Option<String>,
    /// Any additional fields for personal implementation
    #[serde(flatten)]
    pub extra: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ErrorCode {
    Timeout = 0,
    NodeNotFound = 1,
    NotSupported = 10,
    TemporarilyUnavailable = 11,
    MalformedMessage = 12,
    Crash = 13,
    Abort = 14,
    KeyDoesNotExist = 20,
    KeyAlreadyExists = 21,
    PreconditionFailed = 22,
    TxnConflict = 30,
    Other = 999,
}
