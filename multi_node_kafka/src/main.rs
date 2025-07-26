use maelstrom::log::Logs;
use maelstrom::{Message, MessageBody};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader};
use tokio::sync::mpsc;

struct Pending {
    client: String,
    client_msg_id: u64,
    acks: usize,
}

struct Node {
    id: String,
    peers: Vec<String>,
    leader: String,
    msg_id: u64,
    next_offset: u64,
    logs: Logs,
    pendings: HashMap<u64, Pending>,
}

impl Node {
    fn new() -> Self {
        Self {
            id: String::new(),
            peers: Vec::new(),
            leader: String::new(),
            msg_id: 0,
            next_offset: 0,
            logs: Logs::new(),
            pendings: HashMap::new(),
        }
    }

    fn quorum(&self) -> usize {
        self.peers.len().div_ceil(2) + 1
    }

    fn handle_init(&mut self, node_id: String, node_ids: Vec<String>) {
        self.id = node_id.clone();
        self.peers = node_ids
            .iter()
            .filter(|p| *p != &self.id)
            .cloned()
            .collect();
        let mut all = node_ids.clone();
        all.sort();
        self.leader = all[0].clone();
    }

    fn handle_send(
        &mut self,
        message: Message,
        msg_id: u64,
        key: String,
        msg: u64,
    ) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        if self.id != self.leader {
            out.push(Message {
                src: self.id.clone(),
                dest: self.leader.clone(),
                body: MessageBody::ForwardSend {
                    msg_id: self.msg_id,
                    orig_src: message.src,
                    orig_msg_id: msg_id,
                    key,
                    msg,
                },
            })
        } else {
            let offset = self.logs.append_local(&key, msg);
            self.next_offset = offset + 1;
            self.pendings.insert(
                offset,
                Pending {
                    client: message.src.clone(),
                    client_msg_id: msg_id,
                    acks: 1,
                },
            );
            for peer in &self.peers {
                out.push(Message {
                    src: self.id.clone(),
                    dest: peer.clone(),
                    body: MessageBody::Replicate {
                        msg_id: self.msg_id,
                        key: key.clone(),
                        msg,
                        offset,
                    },
                })
            }
            if self.quorum() <= 1 {
                out.push(Message {
                    src: self.id.clone(),
                    dest: message.src,
                    body: MessageBody::SendOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                        offset,
                    },
                });
                self.pendings.remove(&offset);
            }
        }
        out
    }

    fn handle(&mut self, message: Message) -> Vec<Message> {
        let mut out = Vec::new();
        self.msg_id += 1;

        match message.body.clone() {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                self.handle_init(node_id, node_ids);
                out.push(Message {
                    src: self.id.clone(),
                    dest: message.src,
                    body: MessageBody::InitOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                    },
                })
            }
            MessageBody::Send { msg_id, key, msg } => {
                let msgs = self.handle_send(message.clone(), msg_id, key.clone(), msg);
                out.extend(msgs);
            }
            MessageBody::ForwardSend {
                msg_id: _,
                orig_src,
                orig_msg_id,
                key,
                msg,
            } => {
                // leader handles forwarded same as `Send`
                // reuse above by recursive call
                let fwd = Message {
                    src: orig_src,
                    dest: self.id.clone(),
                    body: MessageBody::Send {
                        msg_id: orig_msg_id,
                        key,
                        msg,
                    },
                };
                out.extend(self.handle(fwd));
            }
            MessageBody::Replicate {
                msg_id,
                key,
                msg,
                offset,
            } => {
                self.logs.insert_at(&key, offset, msg);
                out.push(Message {
                    src: self.id.clone(),
                    dest: message.src,
                    body: MessageBody::ReplicateOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                        offset,
                    },
                })
            }
            MessageBody::ReplicateOk {
                msg_id: _,
                in_reply_to: _,
                offset,
            } => {
                // Grab quorum once, before get_mut()
                let quorum = self.quorum();
                // Mutably borrow the pending entry and bump acks
                if let Some(p) = self.pendings.get_mut(&offset) {
                    p.acks += 1;
                    // Check against the pre-computed quorum
                    if p.acks >= quorum {
                        // Take ownership of the Pending so we drop the &mut borrow
                        let Pending {
                            client,
                            client_msg_id,
                            ..
                        } = self.pendings.remove(&offset).unwrap();
                        // Now safe to immutably borrow `self` to build the response
                        out.push(Message {
                            src: self.id.clone(),
                            dest: client,
                            body: MessageBody::SendOk {
                                msg_id: self.msg_id,
                                in_reply_to: client_msg_id,
                                offset,
                            },
                        });
                    }
                }
            }
            MessageBody::Poll { msg_id, offsets } => {
                let msgs = self.logs.poll(&offsets);
                out.push(Message {
                    src: self.id.clone(),
                    dest: message.src,
                    body: MessageBody::PollOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                        msgs,
                    },
                })
            }
            MessageBody::CommitOffsets { msg_id, offsets } => {
                self.logs.commit_offsets(offsets);
                out.push(Message {
                    src: self.id.clone(),
                    dest: message.src,
                    body: MessageBody::CommitOffsetsOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                    },
                })
            }
            MessageBody::ListCommittedOffsets { msg_id, keys } => {
                let offsets = self.logs.list_committed_offsets(&keys);
                out.push(Message {
                    src: self.id.clone(),
                    dest: message.src,
                    body: MessageBody::ListCommittedOffsetsOk {
                        msg_id: self.msg_id,
                        in_reply_to: msg_id,
                        offsets,
                    },
                })
            }
            _ => {}
        }

        out
    }
}

#[tokio::main]
async fn main() {
    let mut node = Node::new();
    let (tx, mut rx) = mpsc::channel::<Message>(32);

    // Spawn stdin reader
    let stdin_tx = tx.clone();
    tokio::spawn(async move {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin);
        for line in reader.lines() {
            if let Ok(line) = line {
                if let Ok(msg) = serde_json::from_str::<Message>(&line) {
                    let _ = stdin_tx.send(msg).await;
                }
            }
        }
    });

    while let Some(msg) = rx.recv().await {
        for resp in node.handle(msg) {
            let response_str = serde_json::to_string(&resp).unwrap();
            println!("{response_str}");
        }
    }
}
