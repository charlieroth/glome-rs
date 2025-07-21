use maelstrom::node::Node;
use maelstrom::{Body, BroadcastGossip, BroadcastOk, Envelope, ReadOk, TopologyOk};
use std::collections::HashSet;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader, stdin};
use tokio::sync::mpsc;
use tokio::time::interval;

pub struct MultiNodeBroadcast {
    pub node: Node,
    pub neighbors: Vec<String>,
    pub messages: HashSet<u64>,
}

impl MultiNodeBroadcast {
    async fn spawn(
        sender: mpsc::Sender<Envelope>,
        receiver: mpsc::Receiver<Envelope>,
    ) -> tokio::task::JoinHandle<()> {
        let mut multi_node_broadcast = MultiNodeBroadcast {
            node: Node::new(sender, receiver),
            neighbors: Vec::new(),
            messages: HashSet::new(),
        };
        tokio::spawn(async move {
            multi_node_broadcast.run().await;
        })
    }

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
                            let _ = self.node
                                .send(Envelope {
                                    src: self.node.node_id.clone(),
                                    dest: neighbor.clone(),
                                    body: Body::BroadcastGossip(BroadcastGossip {
                                        msg_id,
                                        messages: self.messages.iter().cloned().collect(),
                                    }),
                                })
                                .await;
                        }
                    }
                }
                // If the Node receives a message, handle message
                Some(env) = self.node.recv() => {
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
                                if let Err(e) = self.node
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
                            if let Err(e) = self.node
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
                            if let Err(e) = self.node
                                .send(Envelope {
                                    src: self.node.node_id.clone(),
                                    dest: env.src,
                                    body: Body::ReadOk(ReadOk {
                                        msg_id,
                                        in_reply_to: read.msg_id,
                                        messages: Some(self.messages.iter().cloned().collect()),
                                        value: None,
                                    }),
                                })
                                .await
                            {
                                eprintln!("Failed to send read response: {e}");
                            }
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

    MultiNodeBroadcast::spawn(tx2, rx).await;

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
