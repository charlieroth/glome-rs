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
    /// Node messages
    messages: Vec<u64>,
}

impl Node {
    fn new() -> Self {
        Self {
            id: String::new(),
            peers: Vec::new(),
            msg_id: 0,
            messages: Vec::new(),
        }
    }

    fn handle_init(&mut self, node_id: String, node_ids: Vec<String>) {
        self.id = node_id.clone();
        self.peers = node_ids.clone();
        self.peers.retain(|p| p != &self.id);
    }

    fn handle_broadcast(&mut self, message: u64) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        self.messages.push(message);
        for peer in self.peers.iter() {
            out.push(Message {
                src: self.id.clone(),
                dest: peer.clone(),
                body: MessageBody::Broadcast {
                    msg_id: self.msg_id,
                    message,
                },
            });
            self.msg_id += 1;
        }
        out
    }

    fn handle_read(&self) -> Vec<u64> {
        self.messages.clone()
    }

    fn handle(&mut self, msg: Message) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        self.msg_id += 1;
        match msg.body.clone() {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                self.handle_init(node_id, node_ids);
                out.push(Message {
                    src: self.id.clone(),
                    dest: msg.src,
                    body: MessageBody::InitOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                    },
                });
            }
            MessageBody::Topology {
                msg_id,
                topology: _,
            } => {
                out.push(Message {
                    src: self.id.clone(),
                    dest: msg.src,
                    body: MessageBody::TopologyOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                    },
                });
            }
            MessageBody::Broadcast { msg_id, message } => {
                let broadcasts = self.handle_broadcast(message);
                out.extend(broadcasts);
                out.push(Message {
                    src: self.id.clone(),
                    dest: msg.src,
                    body: MessageBody::BroadcastOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                    },
                });
            }
            MessageBody::Read { msg_id } => {
                let messages = self.handle_read();
                out.push(Message {
                    src: self.id.clone(),
                    dest: msg.src,
                    body: MessageBody::ReadOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                        messages: Some(messages),
                        value: None,
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
        for response in node.handle(msg) {
            let response_str = serde_json::to_string(&response).unwrap();
            println!("{response_str}");
        }
    }
}
