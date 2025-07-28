use maelstrom::{
    Message, MessageBody,
    node::{MessageHandler, Node, run_node},
};
use rand::Rng;

struct UniqueIdNode;

impl UniqueIdNode {
    fn handle_generate(&self, _msg_id: u64) -> u64 {
        rand::rng().random::<u64>()
    }
}

impl MessageHandler for UniqueIdNode {
    fn handle(&mut self, node: &mut Node, message: Message) -> Vec<Message> {
        let mut out: Vec<Message> = Vec::new();
        match message.body {
            MessageBody::Init {
                msg_id,
                node_id,
                node_ids,
            } => {
                node.handle_init(node_id, node_ids);
                out.push(node.init_ok(message.src, msg_id));
            }
            MessageBody::Generate { msg_id } => {
                let unique_id = self.handle_generate(msg_id);
                let response_msg_id = node.next_msg_id();
                out.push(node.reply(
                    message.src,
                    MessageBody::GenerateOk {
                        msg_id: response_msg_id,
                        in_reply_to: msg_id,
                        id: unique_id,
                    },
                ));
            }
            _ => {}
        }
        out
    }
}

#[tokio::main]
async fn main() {
    run_node(UniqueIdNode).await;
}
