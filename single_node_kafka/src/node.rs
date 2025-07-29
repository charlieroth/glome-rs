use maelstrom::simple_log::Logs;
use maelstrom::{
    Message, MessageBody,
    node::{MessageHandler, Node},
};

pub struct KafkaNode {
    /// Append-only logs
    logs: Logs,
}

impl KafkaNode {
    pub fn new() -> Self {
        Self { logs: Logs::new() }
    }
}

impl MessageHandler for KafkaNode {
    fn handle(&mut self, node: &mut Node, message: Message) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        match message.body.clone() {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                node.handle_init(node_id, node_ids);
                out.push(node.init_ok(message.src, msg_id));
            }
            MessageBody::Send { msg_id, key, msg } => {
                let offset = self.logs.append(&key, msg);
                let reply_msg_id = node.next_msg_id();
                out.push(node.reply(
                    message.src,
                    MessageBody::SendOk {
                        msg_id: reply_msg_id,
                        in_reply_to: msg_id,
                        offset,
                    },
                ));
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
                ));
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
                ));
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
                ));
            }
            _ => {}
        }
        out
    }
}
