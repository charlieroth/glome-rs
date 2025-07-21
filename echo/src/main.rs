use maelstrom::{Body, EchoOk, Envelope, InitOk};
use tokio::io::{AsyncBufReadExt, BufReader, stdin};
use tokio::sync::mpsc;

pub struct Node {
    pub msg_id: u64,
    pub node_id: String,
    pub node_ids: Vec<String>,
    pub sender: mpsc::Sender<Envelope>,
    pub receiver: mpsc::Receiver<Envelope>,
}

impl Node {
    async fn new(
        sender: mpsc::Sender<Envelope>,
        receiver: mpsc::Receiver<Envelope>,
    ) -> tokio::task::JoinHandle<()> {
        let mut node = Node {
            msg_id: 0,
            node_id: String::new(),
            node_ids: Vec::new(),
            receiver,
            sender,
        };
        tokio::spawn(async move {
            node.run().await;
        })
    }

    async fn run(&mut self) {
        while let Some(env) = self.receiver.recv().await {
            match env.body {
                Body::Init(init) => {
                    self.msg_id += 1;
                    self.node_id = init.node_id;
                    self.node_ids = init.node_ids;
                    self.sender
                        .send(Envelope {
                            src: self.node_id.clone(),
                            dest: env.src,
                            body: Body::InitOk(InitOk {
                                msg_id: self.msg_id,
                                in_reply_to: init.msg_id,
                            }),
                        })
                        .await
                        .unwrap();
                }
                Body::Echo(echo) => {
                    self.msg_id += 1;

                    self.sender
                        .send(Envelope {
                            src: self.node_id.clone(),
                            dest: env.src,
                            body: Body::EchoOk(EchoOk {
                                msg_id: self.msg_id,
                                in_reply_to: echo.msg_id,
                                echo: echo.echo,
                            }),
                        })
                        .await
                        .unwrap();
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

    Node::new(tx2, rx).await;

    let mut lines = BufReader::new(stdin()).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        match serde_json::from_str::<Envelope>(&line) {
            Ok(env) => match tx.send(env).await {
                Ok(()) => {}
                Err(error) => eprintln!("message failed to send: {}", error),
            },
            Err(error) => {
                eprintln!("failed to parse incoming message: {}", error);
                eprintln!("input line was: {}", line);
                std::process::exit(1);
            }
        }
    }
}
