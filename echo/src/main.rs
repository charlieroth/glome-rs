use echo::node::EchoNode;
use maelstrom::node::run_node;

#[tokio::main]
async fn main() {
    run_node(EchoNode).await;
}
