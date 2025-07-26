use maelstrom::simple_log::Logs;
use maelstrom::{Message, MessageBody};
use tokio::{
    io::{self, AsyncBufReadExt, BufReader},
    sync::mpsc,
};

struct Node {
    /// Unique node identifier
    id: String,
    /// Peer node IDs for gossip
    peers: Vec<String>,
    /// Message counter for generating unique msg_ids
    msg_id: u64,
    /// Append-only logs
    logs: Logs,
}

impl Node {
    fn new() -> Self {
        Self {
            id: String::new(),
            peers: Vec::new(),
            msg_id: 0,
            logs: Logs::new(),
        }
    }

    fn handle_init(&mut self, node_id: String, node_ids: Vec<String>) {
        self.id = node_id.clone();
        self.peers = node_ids.clone();
        self.peers.retain(|p| p != &self.id);
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
            MessageBody::Send { msg_id, key, msg } => {
                let offset = self.logs.append(&key, msg);
                out.push(Message {
                    src: self.id.clone(),
                    dest: message.src,
                    body: MessageBody::SendOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                        offset,
                    },
                })
            }
            MessageBody::Poll { msg_id, offsets } => {
                let msgs = self.logs.poll(&offsets);
                out.push(Message {
                    src: self.id.clone(),
                    dest: message.src,
                    body: MessageBody::PollOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                        msgs,
                    },
                })
            }
            MessageBody::CommitOffsets { msg_id, offsets } => {
                self.logs.commit_offsets(offsets);
                out.push(Message {
                    src: self.id.clone(),
                    dest: message.src,
                    body: MessageBody::CommitOffsetsOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                    },
                })
            }
            MessageBody::ListCommittedOffsets { msg_id, keys } => {
                let offsets = self.logs.list_committed_offsets(&keys);
                out.push(Message {
                    src: self.id.clone(),
                    dest: message.src,
                    body: MessageBody::ListCommittedOffsetsOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                        offsets,
                    },
                })
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
