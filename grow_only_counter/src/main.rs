use maelstrom::{
    AddOk, Body, CounterGossip, Envelope, ReadOk, TopologyOk,
    kv::KV,
    node::{Node, NodeExt},
};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader, stdin};
use tokio::sync::mpsc;
use tokio::time::interval;

pub struct GrowOnlyCounterNode {
    pub node: Node,
    pub kv: KV,
}

impl GrowOnlyCounterNode {
    pub fn new(sender: mpsc::Sender<Envelope>, receiver: mpsc::Receiver<Envelope>) -> Self {
        Self {
            node: Node::new(sender, receiver),
            kv: KV::new(),
        }
    }

    async fn spawn(
        sender: mpsc::Sender<Envelope>,
        receiver: mpsc::Receiver<Envelope>,
    ) -> tokio::task::JoinHandle<()> {
        let mut node = GrowOnlyCounterNode::new(sender, receiver);
        tokio::spawn(async move {
            node.run().await;
        })
    }
}

impl NodeExt for GrowOnlyCounterNode {
    async fn run(&mut self) {
        let mut gossip_timer = interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                _ = gossip_timer.tick() => {
                    if !self.node.node_id.is_empty() && !self.node.node_ids.is_empty() {
                        if self.kv.counters.is_empty() {
                            continue;
                        }

                        let node_ids = self.node.node_ids.clone();
                        let current_node_id = self.node.node_id.clone();
                        for node_id in node_ids.iter() {
                            if *node_id == current_node_id {
                                continue
                            }

                            let msg_id = self.node.next_msg_id();
                            let _ = self.node.send(Envelope {
                                src: self.node.node_id.clone(),
                                dest: node_id.clone(),
                                body: Body::CounterGossip(CounterGossip{
                                    msg_id,
                                    counters: self.kv.counters.clone(),
                                })
                            }).await;
                        }
                    }
                }
                Some(env) = self.node.recv() => {
                    match env.body {
                        Body::Init(init) => {
                            self.kv.init(init.node_ids.clone());
                            let _ = self.node.handle_init(init, env.src).await;
                        }
                        Body::Add(add) => {
                            let msg_id = self.node.next_msg_id();
                            self.kv.add(self.node.node_id.clone(), add.delta);
                            let _ = self.node.send(Envelope {
                                src: self.node.node_id.clone(),
                                dest: env.src,
                                body: Body::AddOk(AddOk {
                                    msg_id,
                                    in_reply_to: add.msg_id,
                                }),
                            }).await;
                        }
                        Body::Read(read) => {
                            let msg_id = self.node.next_msg_id();
                            let value = self.kv.read();
                            let _ = self.node.send(Envelope {
                                src: self.node.node_id.clone(),
                                dest: env.src,
                                body: Body::ReadOk(ReadOk {
                                    msg_id,
                                    in_reply_to: read.msg_id,
                                    value: Some(value),
                                    messages: None,
                                }),
                            }).await;
                        }
                        Body::CounterGossip(counter_gossip) => self.kv.merge(counter_gossip.counters),
                        Body::Topology(topology) => {
                            let msg_id = self.node.next_msg_id();
                            let _ = self.node.send(Envelope {
                                src: self.node.node_id.clone(),
                                dest: env.src,
                                body: Body::TopologyOk(TopologyOk {
                                    msg_id,
                                    in_reply_to: topology.msg_id,
                                }),
                            }).await;
                        }
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

    GrowOnlyCounterNode::spawn(tx2, rx).await;

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
