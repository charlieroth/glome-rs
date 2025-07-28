use maelstrom::{
    Message, MessageBody,
    node::{MessageHandler, Node, run_node},
};

struct EchoNode;

impl MessageHandler for EchoNode {
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
            MessageBody::Echo { msg_id, echo } => {
                let response_msg_id = node.next_msg_id();
                out.push(node.reply(
                    message.src,
                    MessageBody::EchoOk {
                        msg_id: response_msg_id,
                        in_reply_to: msg_id,
                        echo,
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
    run_node(EchoNode).await;
}
