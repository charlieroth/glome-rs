use maelstrom::{Message, MessageBody};
use std::io::{self, BufRead, BufReader, Write};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};

struct Node {
    id: RwLock<String>,
    peers: RwLock<Vec<String>>,
    msg_id: RwLock<u64>,
}

impl Node {
    fn new() -> Self {
        Self {
            id: RwLock::new(String::new()),
            peers: RwLock::new(Vec::new()),
            msg_id: RwLock::new(0),
        }
    }

    async fn handle_init(&self, node_id: String, node_ids: Vec<String>) {
        *self.id.write().await = node_id.clone();
        *self.peers.write().await = node_ids
            .iter()
            .filter(|id| *id != &node_id)
            .cloned()
            .collect();
    }

    async fn process_message(&self, msg: Message) -> Option<Message> {
        *self.msg_id.write().await += 1;
        match msg.body {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                self.handle_init(node_id, node_ids).await;
                Some(Message {
                    src: self.id.read().await.clone(),
                    dest: msg.src,
                    body: MessageBody::InitOk {
                        msg_id: *self.msg_id.read().await,
                        in_reply_to: msg_id,
                    },
                })
            }
            MessageBody::Echo { msg_id, echo } => Some(Message {
                src: self.id.read().await.clone(),
                dest: msg.src,
                body: MessageBody::EchoOk {
                    msg_id: *self.msg_id.read().await,
                    in_reply_to: msg_id,
                    echo,
                },
            }),
            _ => None,
        }
    }
}

#[tokio::main]
async fn main() {
    let node = Arc::new(Node::new());
    let (tx, mut rx) = mpsc::channel::<Message>(1000);

    // Spawn stdin reader
    let stdin_tx = tx.clone();
    tokio::spawn(async move {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin);
        for line in reader.lines() {
            if let Ok(line) = line {
                eprintln!("Received: {line}");
                if let Ok(msg) = serde_json::from_str::<Message>(&line) {
                    let _ = stdin_tx.send(msg).await;
                }
            }
        }
    });

    let stdout = io::stdout();
    while let Some(msg) = rx.recv().await {
        let node_clone = node.clone();
        if let Some(response) = node_clone.process_message(msg).await {
            let response_str = serde_json::to_string(&response).unwrap();
            eprintln!("Sending: {response_str}");
            let mut stdout = stdout.lock();
            writeln!(stdout, "{response_str}").unwrap();
            stdout.flush().unwrap();
        }
    }
}
