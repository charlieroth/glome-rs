use maelstrom::node::run_node;
use tarut::node::TarutNode;

#[tokio::main]
async fn main() {
    let handler = TarutNode::new();
    run_node(handler).await;
}
