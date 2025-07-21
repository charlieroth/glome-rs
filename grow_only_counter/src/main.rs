use maelstrom::{AddOk, Body, CounterGossip, Envelope, InitOk, ReadOk, TopologyOk, kv::KV};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader, stdin};
use tokio::sync::mpsc;
use tokio::time::interval;

pub struct Node {
    pub msg_id: u64,
    pub node_id: String,
    pub node_ids: Vec<String>,
    pub neighbors: Vec<String>,
    pub kv: KV,
    pub sender: mpsc::Sender<Envelope>,
    pub receiver: mpsc::Receiver<Envelope>,
}

impl Node {
    async fn spawn(
        sender: mpsc::Sender<Envelope>,
        receiver: mpsc::Receiver<Envelope>,
    ) -> tokio::task::JoinHandle<()> {
        let mut node = Node {
            msg_id: 0,
            node_id: String::new(),
            node_ids: Vec::new(),
            neighbors: Vec::new(),
            kv: KV::new(),
            receiver,
            sender,
        };
        tokio::spawn(async move {
            node.run().await;
        })
    }

    async fn run(&mut self) {
        let mut gossip_timer = interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                _ = gossip_timer.tick() => {
                    if !self.node_id.is_empty() && !self.node_ids.is_empty() {
                        if self.kv.counters.is_empty() {
                            continue;
                        }

                        for node_id in self.node_ids.iter() {
                            if *node_id == self.node_id {
                                continue
                            }

                            self.msg_id += 1;
                            let _ = self.sender.send(Envelope {
                                src: self.node_id.clone(),
                                dest: node_id.clone(),
                                body: Body::CounterGossip(CounterGossip{
                                    msg_id: self.msg_id,
                                    counters: self.kv.counters.clone(),
                                })
                            }).await;
                        }
                    }
                }
                Some(env) = self.receiver.recv() => {
                    match env.body {
                        Body::Init(init) => {
                            self.msg_id += 1;
                            self.node_id = init.node_id;
                            self.node_ids = init.node_ids;

                            self.kv.init(self.node_ids.clone());

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
                        Body::Add(add) => {
                            self.msg_id += 1;
                            self.kv.add(self.node_id.clone(), add.delta);
                            self.sender
                                .send(Envelope {
                                    src: self.node_id.clone(),
                                    dest: env.src,
                                    body: Body::AddOk(AddOk {
                                        msg_id: self.msg_id,
                                        in_reply_to: add.msg_id,
                                    }),
                                })
                                .await
                                .unwrap();
                        }
                        Body::Read(read) => {
                            self.msg_id += 1;
                            let value = self.kv.read();
                            self.sender
                                .send(Envelope {
                                    src: self.node_id.clone(),
                                    dest: env.src,
                                    body: Body::ReadOk(ReadOk {
                                        msg_id: self.msg_id,
                                        in_reply_to: read.msg_id,
                                        value: Some(value),
                                        messages: None,
                                    }),
                                })
                                .await
                                .unwrap();
                        }
                        Body::CounterGossip(counter_gossip) => self.kv.merge(counter_gossip.counters),
                        Body::Topology(topology) => {
                            self.msg_id += 1;
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

    Node::spawn(tx2, rx).await;

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
