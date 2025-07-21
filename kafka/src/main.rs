use maelstrom::{
    Body, CommitOffsetsOk, Envelope, InitOk, ListCommittedOffsetsOk, PollOk, SendOk, TopologyOk,
    log::{Log, LogEntry},
};
use tokio::io::{AsyncBufReadExt, BufReader, stdin};
use tokio::sync::mpsc;

pub struct Node {
    pub msg_id: u64,
    pub node_id: String,
    pub node_ids: Vec<String>,
    pub log: Log,
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
            log: Log::new(),
            receiver,
            sender,
        };
        tokio::spawn(async move {
            node.run().await;
        })
    }

    async fn run(&mut self) {
        loop {
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
                    Body::Send(send) => {
                        self.msg_id += 1;
                        let offset = self.log.inc_offset(send.key.clone());
                        let entry = LogEntry {
                            offset,
                            msg: send.msg,
                        };
                        self.log.append(send.key, entry);
                        self.sender
                            .send(Envelope {
                                src: self.node_id.clone(),
                                dest: env.src,
                                body: Body::SendOk(SendOk {
                                    msg_id: self.msg_id,
                                    in_reply_to: send.msg_id,
                                    offset,
                                }),
                            })
                            .await
                            .unwrap();
                    }
                    Body::Poll(poll) => {
                        self.msg_id += 1;
                        self.sender
                            .send(Envelope {
                                src: self.node_id.clone(),
                                dest: env.src,
                                body: Body::PollOk(PollOk {
                                    msg_id: self.msg_id,
                                    in_reply_to: poll.msg_id,
                                    msgs: self.log.logs(poll.offsets),
                                }),
                            })
                            .await
                            .unwrap();
                    }
                    Body::CommitOffsets(commit_offsets) => {
                        self.msg_id += 1;
                        self.log.set_commit_offsets(commit_offsets.offsets);
                        self.sender
                            .send(Envelope {
                                src: self.node_id.clone(),
                                dest: env.src,
                                body: Body::CommitOffsetsOk(CommitOffsetsOk {
                                    msg_id: self.msg_id,
                                    in_reply_to: commit_offsets.msg_id,
                                }),
                            })
                            .await
                            .unwrap();
                    }
                    Body::ListCommittedOffsets(list_commit_offsets) => {
                        self.msg_id += 1;
                        self.sender
                            .send(Envelope {
                                src: self.node_id.clone(),
                                dest: env.src,
                                body: Body::ListCommittedOffsetsOk(ListCommittedOffsetsOk {
                                    msg_id: self.msg_id,
                                    in_reply_to: list_commit_offsets.msg_id,
                                    offsets: self.log.commits.clone(),
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
