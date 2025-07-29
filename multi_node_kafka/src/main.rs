use maelstrom::log::Logs;
use maelstrom::{
    Message, MessageBody,
    node::{MessageHandler, Node, run_node},
};
use std::collections::HashMap;

struct Pending {
    client: String,
    client_msg_id: u64,
    acks: usize,
}

struct KafkaNode {
    /// Current leader node ID in the cluster
    leader: String,
    /// Next offset for node to use
    next_offset: u64,
    /// Append-only logs
    logs: Logs,
    /// Pending operations
    pendings: HashMap<u64, Pending>,
}

impl KafkaNode {
    fn new() -> Self {
        Self {
            leader: String::new(),
            next_offset: 0,
            logs: Logs::new(),
            pendings: HashMap::new(),
        }
    }

    fn quorum(&self, node: &Node) -> usize {
        node.peers.len().div_ceil(2) + 1
    }

    fn handle_init(&mut self, node: &mut Node, node_id: String, node_ids: Vec<String>) {
        node.handle_init(node_id.clone(), node_ids.clone());
        let mut all = node_ids.clone();
        all.sort();
        self.leader = all[0].clone();
    }

    fn handle_send(
        &mut self,
        node: &mut Node,
        message: Message,
        msg_id: u64,
        key: String,
        msg: u64,
    ) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        if node.id != self.leader {
            out.push(Message {
                src: node.id.clone(),
                dest: self.leader.clone(),
                body: MessageBody::ForwardSend {
                    msg_id: node.next_msg_id(),
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
            let peers = node.peers.clone();
            for peer in peers {
                let msg_id = node.next_msg_id();
                out.push(Message {
                    src: node.id.clone(),
                    dest: peer,
                    body: MessageBody::Replicate {
                        msg_id,
                        key: key.clone(),
                        msg,
                        offset,
                    },
                })
            }
            if self.quorum(node) <= 1 {
                out.push(Message {
                    src: node.id.clone(),
                    dest: message.src,
                    body: MessageBody::SendOk {
                        msg_id: node.next_msg_id(),
                        in_reply_to: msg_id,
                        offset,
                    },
                });
                self.pendings.remove(&offset);
            }
        }
        out
    }
}

impl MessageHandler for KafkaNode {
    fn handle(&mut self, node: &mut Node, message: Message) -> Vec<Message> {
        let mut out = Vec::new();
        match message.body.clone() {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                self.handle_init(node, node_id, node_ids);
                out.push(node.init_ok(message.src, msg_id));
            }
            MessageBody::Send { msg_id, key, msg } => {
                let msgs = self.handle_send(node, message.clone(), msg_id, key.clone(), msg);
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
                    dest: node.id.clone(),
                    body: MessageBody::Send {
                        msg_id: orig_msg_id,
                        key,
                        msg,
                    },
                };
                out.extend(self.handle(node, fwd));
            }
            MessageBody::Replicate {
                msg_id,
                key,
                msg,
                offset,
            } => {
                self.logs.insert_at(&key, offset, msg);
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    message.src,
                    MessageBody::ReplicateOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                        offset,
                    },
                ))
            }
            MessageBody::ReplicateOk {
                msg_id: _,
                in_reply_to: _,
                offset,
            } => {
                // Grab quorum once, before get_mut()
                let quorum = self.quorum(node);
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
                        let reply_msg_id = node.next_msg_id();
                        out.push(node.reply(
                            client,
                            MessageBody::SendOk {
                                msg_id: reply_msg_id,
                                in_reply_to: client_msg_id,
                                offset,
                            },
                        ));
                    }
                }
            }
            MessageBody::Poll { msg_id, offsets } => {
                let msgs = self.logs.poll(&offsets);
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    message.src,
                    MessageBody::PollOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                        msgs,
                    },
                ))
            }
            MessageBody::CommitOffsets { msg_id, offsets } => {
                self.logs.commit_offsets(offsets);
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    message.src,
                    MessageBody::CommitOffsetsOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                    },
                ))
            }
            MessageBody::ListCommittedOffsets { msg_id, keys } => {
                let offsets = self.logs.list_committed_offsets(&keys);
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    message.src,
                    MessageBody::ListCommittedOffsetsOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                        offsets,
                    },
                ))
            }
            _ => {}
        }
        out
    }
}

#[tokio::main]
async fn main() {
    let handler = KafkaNode::new();
    run_node(handler).await;
}
