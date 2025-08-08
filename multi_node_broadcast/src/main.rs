use maelstrom::{
    Message,
    node::{MessageHandler, Node},
};
use multi_node_broadcast::node::MultiNodeBroadcastNode;
use std::io::Write as _;
use tokio::{
    io::{self, AsyncBufReadExt, BufReader},
    sync::mpsc,
    time::{Duration, interval},
};

#[tokio::main]
async fn main() {
    let mut handler = MultiNodeBroadcastNode::new();
    let mut node = Node::new();
    let (tx, mut rx) = mpsc::channel::<Message>(32);
    let mut gossip_timer = interval(Duration::from_millis(100));

    // Spawn stdin reader
    let stdin_tx = tx.clone();
    tokio::spawn(async move {
        let reader = BufReader::new(io::stdin());
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Ok(msg) = serde_json::from_str::<Message>(&line) {
                let _ = stdin_tx.send(msg).await;
            }
        }
    });

    loop {
        tokio::select! {
            _ = gossip_timer.tick() => {
                let msgs = handler.gossip(&mut node);
                for msg in msgs {
                    match serde_json::to_vec(&msg) {
                        Ok(mut bytes) => {
                            bytes.push(b'\n');
                            if let Err(e) = std::io::stdout().write_all(&bytes) {
                                eprintln!("stdout write error: {e:?} for response: {:?}", msg);
                            }
                        }
                        Err(e) => {
                            eprintln!("serialize error: {e:?} for response: {:?}", msg);
                        }
                    }
                }
            }
            Some(msg) = rx.recv() => {
                for response in handler.handle(&mut node, msg) {
                    match serde_json::to_vec(&response) {
                        Ok(mut bytes) => {
                            bytes.push(b'\n');
                            if let Err(e) = std::io::stdout().write_all(&bytes) {
                                eprintln!("stdout write error: {e:?} for response: {:?}", response);
                            }
                        }
                        Err(e) => {
                            eprintln!("serialize error: {e:?} for response: {:?}", response);
                        }
                    }
                }
            }
        }
    }
}
