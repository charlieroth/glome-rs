use maelstrom::{
    Message, MessageBody,
    MessageHandler, Node,
};

pub struct EchoNode;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_echo_node_handles_init_message() {
        let mut handler = EchoNode;
        let mut node = Node::new();
        
        let init_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Init {
                msg_id: 1,
                node_id: "n1".to_string(),
                node_ids: vec!["n1".to_string(), "n2".to_string(), "n3".to_string()],
            },
        };

        let responses = handler.handle(&mut node, init_message);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n1");
        assert_eq!(responses[0].dest, "c1");
        
        match &responses[0].body {
            MessageBody::InitOk { msg_id: _, in_reply_to } => {
                assert_eq!(*in_reply_to, 1);
            }
            _ => panic!("Expected InitOk message"),
        }

        // Verify node state was updated
        assert_eq!(node.id, "n1");
        assert_eq!(node.peers, vec!["n2", "n3"]);
    }

    #[test]
    fn test_echo_node_handles_echo_message() {
        let mut handler = EchoNode;
        let mut node = Node::new();
        
        // Initialize node first
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        let echo_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Echo {
                msg_id: 42,
                echo: "Hello, World!".to_string(),
            },
        };

        let responses = handler.handle(&mut node, echo_message);

        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].src, "n1");
        assert_eq!(responses[0].dest, "c1");
        
        match &responses[0].body {
            MessageBody::EchoOk { msg_id: _, in_reply_to, echo } => {
                assert_eq!(*in_reply_to, 42);
                assert_eq!(echo, "Hello, World!");
            }
            _ => panic!("Expected EchoOk message"),
        }
    }

    #[test]
    fn test_echo_node_ignores_unknown_messages() {
        let mut handler = EchoNode;
        let mut node = Node::new();
        
        let unknown_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Generate { msg_id: 1 },
        };

        let responses = handler.handle(&mut node, unknown_message);

        assert_eq!(responses.len(), 0);
    }

    #[test]
    fn test_echo_node_multiple_echo_messages() {
        let mut handler = EchoNode;
        let mut node = Node::new();
        
        // Initialize node first
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        // First echo
        let echo1 = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Echo {
                msg_id: 1,
                echo: "First".to_string(),
            },
        };

        let responses1 = handler.handle(&mut node, echo1);
        assert_eq!(responses1.len(), 1);

        // Second echo
        let echo2 = Message {
            src: "c2".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Echo {
                msg_id: 2,
                echo: "Second".to_string(),
            },
        };

        let responses2 = handler.handle(&mut node, echo2);
        assert_eq!(responses2.len(), 1);

        // Verify both responses are correct
        match &responses1[0].body {
            MessageBody::EchoOk { msg_id: _, in_reply_to, echo } => {
                assert_eq!(*in_reply_to, 1);
                assert_eq!(echo, "First");
            }
            _ => panic!("Expected EchoOk message"),
        }

        match &responses2[0].body {
            MessageBody::EchoOk { msg_id: _, in_reply_to, echo } => {
                assert_eq!(*in_reply_to, 2);
                assert_eq!(echo, "Second");
            }
            _ => panic!("Expected EchoOk message"),
        }
    }

    #[test]
    fn test_echo_node_generates_unique_msg_ids() {
        let mut handler = EchoNode;
        let mut node = Node::new();
        
        // Initialize node first
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        let echo_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Echo {
                msg_id: 1,
                echo: "test".to_string(),
            },
        };

        let responses1 = handler.handle(&mut node, echo_message.clone());
        let responses2 = handler.handle(&mut node, echo_message);

        // Extract msg_ids from responses
        let msg_id1 = match &responses1[0].body {
            MessageBody::EchoOk { msg_id, .. } => *msg_id,
            _ => panic!("Expected EchoOk message"),
        };

        let msg_id2 = match &responses2[0].body {
            MessageBody::EchoOk { msg_id, .. } => *msg_id,
            _ => panic!("Expected EchoOk message"),
        };

        assert_ne!(msg_id1, msg_id2);
        assert_eq!(msg_id2, msg_id1 + 1);
    }
}
