use maelstrom::log::Logs;
use maelstrom::{Message, MessageBody};
use std::io::{self, BufRead, BufReader};
use tokio::sync::mpsc;

struct Node {
    id: String,
    peers: Vec<String>,
    msg_id: u64,
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
            MessageBody::Send { msg_id, key, msg } => {
                let offset = self.logs.append(&key, msg);
                Some(Message {
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
                Some(Message {
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
                Some(Message {
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
                Some(Message {
                    src: self.id.clone(),
                    dest: message.src,
                    body: MessageBody::ListCommittedOffsetsOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                        offsets
                    }
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
