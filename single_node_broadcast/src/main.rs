use maelstrom::run_node;
use single_node_broadcast::node::SingleNodeBroadcastNode;

#[tokio::main]
async fn main() {
    let handler = SingleNodeBroadcastNode::new();
    run_node(handler).await;
}
