use maelstrom::run_node;
use single_node_tat::node::TatNode;

#[tokio::main]
async fn main() {
    let handler = TatNode::new();
    run_node(handler).await;
}
