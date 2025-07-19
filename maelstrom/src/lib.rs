use serde::{Deserialize, Serialize};
use serde_json::Value;

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
