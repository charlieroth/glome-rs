use maelstrom::node::{Node, NodeExt};
use maelstrom::{Body, BroadcastGossip, BroadcastOk, Envelope, ReadOk, TopologyOk};
use rand::seq::SliceRandom;
use std::collections::HashSet;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader, stdin};
use tokio::sync::mpsc;
use tokio::time::interval;

pub struct EfficientBroadcastNode {
    pub node: Node,
    pub neighbors: Vec<String>,
    pub messages: HashSet<u64>,
}

impl EfficientBroadcastNode {
    fn new(sender: mpsc::Sender<Envelope>, receiver: mpsc::Receiver<Envelope>) -> Self {
        Self {
            node: Node::new(sender, receiver),
            neighbors: Vec::new(),
            messages: HashSet::new(),
        }
    }

    async fn spawn(
        sender: mpsc::Sender<Envelope>,
        receiver: mpsc::Receiver<Envelope>,
    ) -> tokio::task::JoinHandle<()> {
        let mut node = EfficientBroadcastNode::new(sender, receiver);
        tokio::spawn(async move {
            node.run().await;
        })
    }

    fn construct_k_regular_neighbors(&self, k: usize) -> Vec<String> {
        let mut rng = rand::rng();
        let mut other_nodes: Vec<String> = self
            .node
            .node_ids
            .iter()
            .filter(|&node| node != &self.node.node_id)
            .cloned()
            .collect();

        other_nodes.shuffle(&mut rng);
        let len = other_nodes.len();
        other_nodes.into_iter().take(k.min(len)).collect()
    }
}

impl NodeExt for EfficientBroadcastNode {
    async fn run(&mut self) {
        // Setup periodic gossip interval
        let mut gossip_timer = interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                // If the gossip timer goes off, gossip current set of messages to Node's neighbors
                _ = gossip_timer.tick() => {
                    if !self.node.node_id.is_empty() && !self.neighbors.is_empty() && !self.messages.is_empty() {
                        for neighbor in self.neighbors.iter() {
                            if neighbor == &self.node.node_id {
                                continue;
                            }

                            let msg_id = self.node.next_msg_id();
                            let _ = self.node.send(Envelope {
                                src: self.node.node_id.clone(),
                                dest: neighbor.clone(),
                                body: Body::BroadcastGossip(BroadcastGossip {
                                    msg_id,
                                    messages: self.messages.iter().cloned().collect(),
                                }),
                            }).await;
                        }
                    }
                }
                // If the Node receives a message, handle message
                Some(env) = self.node.recv() => {
                    match env.body {
                        Body::Init(init) => {
                            self.node.handle_init(init, env.src).await.unwrap();

                            // Construct random k-regular graph topology
                            self.neighbors = self.construct_k_regular_neighbors(4);
                        }
                        Body::BroadcastGossip(broadcast_gossip) => {
                            for message in broadcast_gossip.messages {
                                self.messages.insert(message);
                            }
                        }
                        Body::Broadcast(broadcast) => {
                            // Add incoming message to Node's messages
                            self.messages.insert(broadcast.message);

                            // Reply to broadcast message
                            let msg_id = self.node.next_msg_id();
                            self.node.send(Envelope {
                                src: self.node.node_id.clone(),
                                dest: env.src,
                                body: Body::BroadcastOk(BroadcastOk {
                                    msg_id,
                                    in_reply_to: broadcast.msg_id,
                                }),
                            }).await.unwrap();
                        }
                        Body::Read(read) => {
                            let msg_id = self.node.next_msg_id();
                            self.node.send(Envelope {
                                src: self.node.node_id.clone(),
                                dest: env.src,
                                body: Body::ReadOk(ReadOk {
                                    msg_id,
                                    in_reply_to: read.msg_id,
                                    messages: Some(self.messages.iter().cloned().collect()),
                                    value: None,
                                }),
                            }).await.unwrap();
                        }
                        Body::Topology(topology) => {
                            let msg_id = self.node.next_msg_id();
                            self.node.send(Envelope {
                                src: self.node.node_id.clone(),
                                dest: env.src,
                                body: Body::TopologyOk(TopologyOk {
                                    msg_id,
                                    in_reply_to: topology.msg_id,
                                }),
                            }).await.unwrap();
                        }
                        Body::BroadcastOk(_) => {}
                        msg => {
                            println!("unknown message received: {msg:?}");
                        }
                    }
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

    EfficientBroadcastNode::spawn(tx2, rx).await;

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
