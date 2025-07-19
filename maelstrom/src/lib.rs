use serde::{Deserialize, Serialize};

pub struct Node {
    pub node_id: String,
    pub node_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitMsgBody {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub msg_id: i64,
    pub node_id: String,
    pub node_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitMsg {
    pub src: String,
    pub dest: String,
    pub body: InitMsgBody,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitOkBody {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub in_reply_to: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InitOk {
    pub src: String,
    pub dest: String,
    pub body: InitOkBody,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EchoMsgBody {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub msg_id: i64,
    pub echo: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EchoMsg {
    pub src: String,
    pub dest: String,
    pub body: EchoMsgBody,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EchoOkBody {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub msg_id: i64,
    pub in_reply_to: i64,
    pub echo: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EchoOk {
    pub src: String,
    pub dest: String,
    pub body: EchoOkBody,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorMsg {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub in_reply_to: i64,
    pub code: ErrorCode,
    pub text: String,
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
