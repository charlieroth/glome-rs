use maelstrom::run_node;
use multi_node_kafka::node::KafkaNode;

#[tokio::main]
async fn main() {
    let handler = KafkaNode::new();
    run_node(handler).await;
}
