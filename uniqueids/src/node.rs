use maelstrom::{
    Message, MessageBody,
    node::{MessageHandler, Node},
};
use rand::Rng;

pub struct UniqueIdNode;

impl UniqueIdNode {
    pub fn handle_generate(&self, _msg_id: u64) -> u64 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_unique_id_node_handles_init_message() {
        let mut handler = UniqueIdNode;
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
    fn test_unique_id_node_ignores_unknown_messages() {
        let mut handler = UniqueIdNode;
        let mut node = Node::new();
        
        let unknown_message = Message {
            src: "c1".to_string(),
            dest: "n1".to_string(),
            body: MessageBody::Echo { 
                msg_id: 1, 
                echo: "test".to_string() 
            },
        };

        let responses = handler.handle(&mut node, unknown_message);

        assert_eq!(responses.len(), 0);
    }

    #[test]
    fn test_unique_id_node_generates_unique_ids_for_many_requests() {
        let mut handler = UniqueIdNode;
        let mut node = Node::new();
        
        // Initialize node first
        node.handle_init("n1".to_string(), vec!["n1".to_string()]);

        let mut generated_ids = HashSet::new();
        
        // Send 100 generate messages and collect all unique IDs
        for i in 0..100 {
            let generate_message = Message {
                src: "c1".to_string(),
                dest: "n1".to_string(),
                body: MessageBody::Generate {
                    msg_id: i,
                },
            };

            let responses = handler.handle(&mut node, generate_message);
            assert_eq!(responses.len(), 1);

            match &responses[0].body {
                MessageBody::GenerateOk { msg_id: _, in_reply_to, id } => {
                    assert_eq!(*in_reply_to, i);
                    // Insert the ID into the set - if it's not unique, insert will return false
                    assert!(generated_ids.insert(*id), "Generated non-unique ID: {id}");
                }
                _ => panic!("Expected GenerateOk message"),
            }
        }

        // Verify we have exactly 100 unique IDs
        assert_eq!(generated_ids.len(), 100);
    }
}
