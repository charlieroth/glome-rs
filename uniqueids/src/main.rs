use maelstrom::{Message, MessageBody};
use std::{
    io::{self, BufRead, BufReader, Write}, sync::Arc
};
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
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
        *self.peers.write().await = node_ids.into_iter().filter(|id| id != &node_id).collect();
    }

    async fn handle_generate(&self, msg_id: u64) -> u64 {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let mut hasher = DefaultHasher::new();
        let node_id = self.id.read().await.clone();
        node_id.hash(&mut hasher);
        msg_id.hash(&mut hasher);
        timestamp.hash(&mut hasher);
        hasher.finish()
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
            MessageBody::Generate { msg_id } => {
                let unique_id = self.handle_generate(msg_id).await;
                Some(Message {
                    src: self.id.read().await.clone(),
                    dest: msg.src,
                    body: MessageBody::GenerateOk {
                        msg_id: *self.msg_id.read().await,
                        in_reply_to: msg_id,
                        id: unique_id
                    }
                })
            }
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

