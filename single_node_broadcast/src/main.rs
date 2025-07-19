use maelstrom::{Body, Broadcast, BroadcastOk, Envelope, InitOk, ReadOk, TopologyOk};
use tokio::io::{AsyncBufReadExt, BufReader, stdin};
use tokio::sync::mpsc;

pub struct Node {
    pub msg_id: u64,
    pub node_id: String,
    pub node_ids: Vec<String>,
    pub neighbors: Vec<String>,
    pub messages: Vec<u64>,
    pub sender: mpsc::Sender<Envelope>,
    pub receiver: mpsc::Receiver<Envelope>,
}

impl Node {
    async fn new(
        sender: mpsc::Sender<Envelope>,
        receiver: mpsc::Receiver<Envelope>,
    ) -> tokio::task::JoinHandle<()> {
        let mut node = Node {
            msg_id: 0,
            node_id: String::new(),
            node_ids: Vec::new(),
            neighbors: Vec::new(),
            messages: Vec::new(),
            receiver,
            sender,
        };
        tokio::spawn(async move {
            node.run().await;
        })
    }

    async fn run(&mut self) {
        while let Some(env) = self.receiver.recv().await {
            match env.body {
                Body::Init(init) => {
                    self.msg_id += 1;
                    self.node_id = init.node_id;
                    self.node_ids = init.node_ids;
                    self.sender
                        .send(Envelope {
                            src: self.node_id.clone(),
                            dest: env.src,
                            body: Body::InitOk(InitOk {
                                msg_id: self.msg_id,
                                in_reply_to: init.msg_id,
                            }),
                        })
                        .await
                        .unwrap();
                }
                Body::Topology(topology) => match topology.topology.get(&self.node_id) {
                    Some(neighbors) => {
                        self.msg_id += 1;
                        self.neighbors = neighbors.clone();
                        self.sender
                            .send(Envelope {
                                src: self.node_id.clone(),
                                dest: env.src,
                                body: Body::TopologyOk(TopologyOk {
                                    msg_id: self.msg_id,
                                    in_reply_to: topology.msg_id,
                                }),
                            })
                            .await
                            .unwrap();
                    }
                    None => {
                        eprintln!("No neighbors found for node: {}", self.node_id);
                    }
                },
                Body::Broadcast(broadcast) => {
                    self.messages.push(broadcast.message);

                    for neighbor in self.neighbors.iter() {
                        self.msg_id += 1;
                        self.sender
                            .send(Envelope {
                                src: self.node_id.clone(),
                                dest: neighbor.clone(),
                                body: Body::Broadcast(Broadcast {
                                    msg_id: self.msg_id,
                                    message: broadcast.message,
                                }),
                            })
                            .await
                            .unwrap();
                    }
                    self.msg_id += 1;
                    self.sender
                        .send(Envelope {
                            src: self.node_id.clone(),
                            dest: env.src,
                            body: Body::BroadcastOk(BroadcastOk {
                                msg_id: self.msg_id,
                                in_reply_to: broadcast.msg_id,
                            }),
                        })
                        .await
                        .unwrap();
                }
                Body::Read(read) => {
                    self.msg_id += 1;
                    self.sender
                        .send(Envelope {
                            src: self.node_id.clone(),
                            dest: env.src,
                            body: Body::ReadOk(ReadOk {
                                msg_id: self.msg_id,
                                in_reply_to: read.msg_id,
                                messages: self.messages.clone(),
                            }),
                        })
                        .await
                        .unwrap();
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

    let _ = tokio::spawn(async move {
        while let Some(env) = rx2.recv().await {
            let reply = serde_json::to_string(&env).unwrap();
            println!("{}", reply);
        }
    });

    let _ = Node::new(tx2, rx).await;

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
