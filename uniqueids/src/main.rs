use maelstrom::node::run_node;
use uniqueids::node::UniqueIdNode;

#[tokio::main]
async fn main() {
    run_node(UniqueIdNode).await;
}
