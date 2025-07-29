use maelstrom::{
    Message, MessageBody,
    node::{MessageHandler, Node},
};

pub struct SingleNodeBroadcastNode {
    /// Node messages
    messages: Vec<u64>,
}

impl Default for SingleNodeBroadcastNode {
    fn default() -> Self {
        Self::new()
    }
}

impl SingleNodeBroadcastNode {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    pub fn handle_broadcast(&mut self, node: &mut Node, message: u64) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        self.messages.push(message);
        for peer in node.peers.clone() {
            out.push(Message {
                src: node.id.clone(),
                dest: peer,
                body: MessageBody::Broadcast {
                    msg_id: node.next_msg_id(),
                    message,
                },
            });
        }
        out
    }

    pub fn handle_read(&self) -> Vec<u64> {
        self.messages.clone()
    }
}

impl MessageHandler for SingleNodeBroadcastNode {
    fn handle(&mut self, node: &mut Node, msg: Message) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        match msg.body.clone() {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                node.handle_init(node_id, node_ids);
                out.push(node.init_ok(msg.src, msg_id));
            }
            MessageBody::Topology {
                msg_id,
                topology: _,
            } => {
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    msg.src,
                    MessageBody::TopologyOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                    },
                ));
            }
            MessageBody::Broadcast { msg_id, message } => {
                let broadcasts = self.handle_broadcast(node, message);
                out.extend(broadcasts);
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    msg.src,
                    MessageBody::BroadcastOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                    },
                ));
            }
            MessageBody::Read { msg_id } => {
                let messages = self.handle_read();
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    msg.src,
                    MessageBody::ReadOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                        messages: Some(messages),
                        value: None,
                    },
                ));
            }
            _ => {}
        }
        out
    }
}
