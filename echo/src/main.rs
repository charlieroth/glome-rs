use echo::node::EchoNode;
use maelstrom::run_node;

#[tokio::main]
async fn main() {
    run_node(EchoNode).await;
}
