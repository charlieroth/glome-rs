use maelstrom::{Message, MessageBody};
use std::io::{self, BufRead, BufReader};
use tokio::sync::mpsc;

struct Node {
    id: String,
    peers: Vec<String>,
    msg_id: u64,
}

impl Node {
    fn new() -> Self {
        Self {
            id: String::new(),
            peers: Vec::new(),
            msg_id: 0,
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

