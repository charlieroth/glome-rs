use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub mod kv;
pub mod log;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub src: String,
    pub dest: String,
    pub body: MessageBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum MessageBody {
    Init {
        msg_id: u64,
        node_id: String,
        node_ids: Vec<String>,
    },
    InitOk {
        msg_id: u64,
        in_reply_to: u64,
    },
    Echo {
        msg_id: u64,
        echo: String,
    },
    EchoOk {
        msg_id: u64,
        in_reply_to: u64,
        echo: String,
    },
    Generate {
        msg_id: u64,
    },
    GenerateOk {
        msg_id: u64,
        in_reply_to: u64,
        id: u64,
    },
    Broadcast {
        msg_id: u64,
        message: u64,
    },
    BroadcastOk {
        msg_id: u64,
        in_reply_to: u64,
    },
    BroadcastGossip {
        msg_id: u64,
        messages: Vec<u64>,
    },
    Read {
        msg_id: u64,
    },
    ReadOk {
        msg_id: u64,
        in_reply_to: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        messages: Option<Vec<u64>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<u64>,
    },
    Topology {
        msg_id: u64,
        topology: HashMap<String, Vec<String>>,
    },
    TopologyOk {
        msg_id: u64,
        in_reply_to: u64,
    },
    Add {
        msg_id: u64,
        delta: u64,
    },
    AddOk {
        msg_id: u64,
        in_reply_to: u64,
    },
    CounterGossip {
        msg_id: u64,
        counters: HashMap<String, kv::Counter>,
    },
    Send {
        msg_id: u64,
        key: String,
        msg: u64,
    },
    SendOk {
        msg_id: u64,
        in_reply_to: u64,
        offset: u64,
    },
    Poll {
        msg_id: u64,
        offsets: HashMap<String, u64>,
    },
    PollOk {
        msg_id: u64,
        in_reply_to: u64,
        msgs: HashMap<String, Vec<(u64, u64)>>,
    },
    CommitOffsets {
        msg_id: u64,
        offsets: HashMap<String, u64>,
    },
    CommitOffsetsOk {
        msg_id: u64,
        in_reply_to: u64,
    },
    ListCommittedOffsets {
        msg_id: u64,
        keys: Vec<String>,
    },
    ListCommittedOffsetsOk {
        msg_id: u64,
        in_reply_to: u64,
        offsets: HashMap<String, u64>,
    },
    Error {
        in_reply_to: u64,
        code: ErrorCode,
        /// Optional human-readable description
        text: Option<String>,
        /// Any additional fields for personal implementation
        #[serde(flatten)]
        extra: Option<Value>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
