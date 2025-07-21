use maelstrom::{Body, Envelope, GenerateOk};
use maelstrom::node::{Node, NodeExt};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tokio::io::{AsyncBufReadExt, BufReader, stdin};
use tokio::sync::mpsc;

pub struct UniqueIdService {
    node: Node,
}

impl UniqueIdService {
    pub fn new(sender: mpsc::Sender<Envelope>, receiver: mpsc::Receiver<Envelope>) -> Self {
        Self {
            node: Node::new(sender, receiver),
        }
    }
}

impl NodeExt for UniqueIdService {
    async fn run(&mut self) {
        while let Some(env) = self.node.recv().await {
            match env.body {
                Body::Init(init) => {
                    if let Err(e) = self.node.handle_init(init, env.src).await {
                        eprintln!("Failed to handle init: {}", e);
                    }
                }
                Body::Generate(generate) => {
                    let msg_id = self.node.next_msg_id();

                    // Create unique ID by combining timestamp, node ID, and message ID
                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos() as u64;
                    let mut hasher = DefaultHasher::new();
                    self.node.node_id.hash(&mut hasher);
                    msg_id.hash(&mut hasher);
                    timestamp.hash(&mut hasher);
                    let unique_id = hasher.finish();

                    let envelope = Envelope {
                        src: self.node.node_id.clone(),
                        dest: env.src,
                        body: Body::GenerateOk(GenerateOk {
                            msg_id,
                            in_reply_to: generate.msg_id,
                            id: unique_id,
                        }),
                    };

                    if let Err(e) = self.node.send(envelope).await {
                        eprintln!("Failed to send response: {}", e);
                    }
                }
                _ => {
                    println!("unknown message received");
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let (tx, rx) = mpsc::channel::<Envelope>(32);
    let (tx2, mut rx2) = mpsc::channel::<Envelope>(32);

    tokio::spawn(async move {
        while let Some(env) = rx2.recv().await {
            let reply = serde_json::to_string(&env).unwrap();
            println!("{}", reply);
        }
    });

    tokio::spawn(async move {
        let mut service = UniqueIdService::new(tx2, rx);
        service.run().await;
    });

    let mut lines = BufReader::new(stdin()).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        match serde_json::from_str::<Envelope>(&line) {
            Ok(env) => match tx.send(env).await {
                Ok(()) => {}
                Err(error) => eprintln!("message failed to send: {}", error),
            },
            Err(error) => {
                eprintln!("failed to parse incoming message: {}", error);
                eprintln!("input line was: {}", line);
                std::process::exit(1);
            }
        }
    }
}
