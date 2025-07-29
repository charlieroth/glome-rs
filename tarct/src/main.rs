use maelstrom::node::run_node;
use tarct::node::TarctNode;

#[tokio::main]
async fn main() {
    let handler = TarctNode::new();
    run_node(handler).await;
}
