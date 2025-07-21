use maelstrom::node::Node;
use maelstrom::{Body, Broadcast, BroadcastOk, Envelope, ReadOk, TopologyOk};
use tokio::io::{AsyncBufReadExt, BufReader, stdin};
use tokio::sync::mpsc;

pub struct BroadcastNode {
    pub node: Node,
    pub neighbors: Vec<String>,
    pub messages: Vec<u64>,
}

impl BroadcastNode {
    async fn spawn(
        sender: mpsc::Sender<Envelope>,
        receiver: mpsc::Receiver<Envelope>,
    ) -> tokio::task::JoinHandle<()> {
        let mut broadcast_node = BroadcastNode {
            node: Node::new(sender, receiver),
            neighbors: Vec::new(),
            messages: Vec::new(),
        };
        tokio::spawn(async move {
            broadcast_node.run().await;
        })
    }

    async fn run(&mut self) {
        while let Some(env) = self.node.recv().await {
            match env.body {
                Body::Init(init) => {
                    if let Err(e) = self.node.handle_init(init, env.src).await {
                        eprintln!("Failed to handle init: {e}");
                    }
                }
                Body::Topology(topology) => match topology.topology.get(&self.node.node_id) {
                    Some(neighbors) => {
                        let msg_id = self.node.next_msg_id();
                        self.neighbors = neighbors.clone();
                        if let Err(e) = self
                            .node
                            .send(Envelope {
                                src: self.node.node_id.clone(),
                                dest: env.src,
                                body: Body::TopologyOk(TopologyOk {
                                    msg_id,
                                    in_reply_to: topology.msg_id,
                                }),
                            })
                            .await
                        {
                            eprintln!("Failed to send topology response: {e}");
                        }
                    }
                    None => {
                        eprintln!("No neighbors found for node: {}", self.node.node_id);
                    }
                },
                Body::Broadcast(broadcast) => {
                    self.messages.push(broadcast.message);

                    for neighbor in self.neighbors.iter() {
                        let msg_id = self.node.next_msg_id();
                        if let Err(e) = self
                            .node
                            .send(Envelope {
                                src: self.node.node_id.clone(),
                                dest: neighbor.clone(),
                                body: Body::Broadcast(Broadcast {
                                    msg_id,
                                    message: broadcast.message,
                                }),
                            })
                            .await
                        {
                            eprintln!("Failed to send broadcast to neighbor: {e}");
                        }
                    }
                    let msg_id = self.node.next_msg_id();
                    if let Err(e) = self
                        .node
                        .send(Envelope {
                            src: self.node.node_id.clone(),
                            dest: env.src,
                            body: Body::BroadcastOk(BroadcastOk {
                                msg_id,
                                in_reply_to: broadcast.msg_id,
                            }),
                        })
                        .await
                    {
                        eprintln!("Failed to send broadcast response: {e}");
                    }
                }
                Body::Read(read) => {
                    let msg_id = self.node.next_msg_id();
                    if let Err(e) = self
                        .node
                        .send(Envelope {
                            src: self.node.node_id.clone(),
                            dest: env.src,
                            body: Body::ReadOk(ReadOk {
                                msg_id,
                                in_reply_to: read.msg_id,
                                messages: Some(self.messages.clone()),
                                value: None,
                            }),
                        })
                        .await
                    {
                        eprintln!("Failed to send read response: {e}");
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
            println!("{reply}");
        }
    });

    BroadcastNode::spawn(tx2, rx).await;

    let mut lines = BufReader::new(stdin()).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        match serde_json::from_str::<Envelope>(&line) {
            Ok(env) => match tx.send(env).await {
                Ok(()) => {}
                Err(error) => eprintln!("message failed to send: {error}"),
            },
            Err(error) => {
                eprintln!("failed to parse incoming message: {error}");
                eprintln!("input line was: {line}");
                std::process::exit(1);
            }
        }
    }
}
