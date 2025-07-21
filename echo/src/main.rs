use maelstrom::node::{Node, NodeExt};
use maelstrom::{Body, EchoOk, Envelope};
use tokio::io::{AsyncBufReadExt, BufReader, stdin};
use tokio::sync::mpsc;

pub struct EchoNode {
    node: Node,
}

impl EchoNode {
    fn new(sender: mpsc::Sender<Envelope>, receiver: mpsc::Receiver<Envelope>) -> Self {
        Self {
            node: Node::new(sender, receiver),
        }
    }

    async fn spawn(
        sender: mpsc::Sender<Envelope>,
        receiver: mpsc::Receiver<Envelope>,
    ) -> tokio::task::JoinHandle<()> {
        let mut echo_node = EchoNode::new(sender, receiver);
        tokio::spawn(async move {
            echo_node.run().await;
        })
    }
}

impl NodeExt for EchoNode {
    async fn run(&mut self) {
        while let Some(env) = self.node.recv().await {
            match env.body {
                Body::Init(init) => {
                    if let Err(e) = self.node.handle_init(init, env.src).await {
                        eprintln!("Failed to handle init: {e}");
                    }
                }
                Body::Echo(echo) => {
                    let msg_id = self.node.next_msg_id();
                    let envelope = Envelope {
                        src: self.node.node_id.clone(),
                        dest: env.src,
                        body: Body::EchoOk(EchoOk {
                            msg_id,
                            in_reply_to: echo.msg_id,
                            echo: echo.echo,
                        }),
                    };
                    if let Err(e) = self.node.send(envelope).await {
                        eprintln!("Failed to send echo response: {e}");
                    }
                }
                _ => {
                    println!("unknown message received");
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let (tx, rx) = mpsc::channel::<Envelope>(32);
    let (tx2, mut rx2) = mpsc::channel::<Envelope>(32);

    tokio::spawn(async move {
        while let Some(env) = rx2.recv().await {
            let reply = serde_json::to_string(&env).unwrap();
            println!("{reply}");
        }
    });

    EchoNode::spawn(tx2, rx).await;

    let mut lines = BufReader::new(stdin()).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        match serde_json::from_str::<Envelope>(&line) {
            Ok(env) => match tx.send(env).await {
                Ok(()) => {}
                Err(error) => eprintln!("message failed to send: {error}"),
            },
            Err(error) => {
                eprintln!("failed to parse incoming message: {error}");
                eprintln!("input line was: {line}");
                std::process::exit(1);
            }
        }
    }
}
