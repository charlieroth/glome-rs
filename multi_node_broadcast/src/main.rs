use std::collections::HashSet;
use std::time::Duration;

use maelstrom::{Body, BroadcastGossip, BroadcastOk, Envelope, InitOk, ReadOk, TopologyOk};
use tokio::io::{AsyncBufReadExt, BufReader, stdin};
use tokio::sync::mpsc;
use tokio::time::interval;

pub struct Node {
    pub msg_id: u64,
    pub node_id: String,
    pub node_ids: Vec<String>,
    pub neighbors: Vec<String>,
    pub messages: HashSet<u64>,
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
            messages: HashSet::new(),
            receiver,
            sender,
        };
        tokio::spawn(async move {
            node.run().await;
        })
    }

    async fn run(&mut self) {
        // Setup periodic gossip interval
        let mut gossip_timer = interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                // If the gossip timer goes off, gossip current set of messages to Node's neighbors
                _ = gossip_timer.tick() => {
                    if !self.node_id.is_empty() && !self.neighbors.is_empty() && !self.messages.is_empty() {
                        for neighbor in self.neighbors.iter() {
                            if neighbor == &self.node_id {
                                continue;
                            }

                            self.msg_id += 1;
                            let _ = self.sender
                                .send(Envelope {
                                    src: self.node_id.clone(),
                                    dest: neighbor.clone(),
                                    body: Body::BroadcastGossip(BroadcastGossip {
                                        msg_id: self.msg_id,
                                        messages: self.messages.iter().cloned().collect(),
                                    }),
                                })
                                .await;
                        }
                    }
                }
                // If the Node receives a message, handle message
                Some(env) = self.receiver.recv() => {
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
                        Body::BroadcastGossip(broadcast_gossip) => {
                            for message in broadcast_gossip.messages {
                                self.messages.insert(message);
                            }
                        }
                        Body::Broadcast(broadcast) => {
                            // Add incoming message to Node's messages
                            self.messages.insert(broadcast.message);

                            // Reply to broadcast message
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
                                        messages: Some(self.messages.iter().cloned().collect()),
                                        value: None,
                                    }),
                                })
                                .await
                                .unwrap();
                        }
                        Body::BroadcastOk(_) => {}
                        msg => {
                            println!("unknown message received: {:?}", msg);
                        }
                    }
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let (inbound, inbox) = mpsc::channel::<Envelope>(32);
    let (outbound, mut outbox) = mpsc::channel::<Envelope>(32);

    tokio::spawn(async move {
        while let Some(env) = outbox.recv().await {
            let reply = serde_json::to_string(&env).unwrap();
            println!("{reply}");
        }
    });

    Node::new(outbound, inbox).await;

    let mut lines = BufReader::new(stdin()).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        match serde_json::from_str::<Envelope>(&line) {
            Ok(env) => match inbound.send(env).await {
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
