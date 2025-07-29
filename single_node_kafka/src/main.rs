use maelstrom::run_node;
use single_node_kafka::node::KafkaNode;

#[tokio::main]
async fn main() {
    let handler = KafkaNode::new();
    run_node(handler).await;
}
