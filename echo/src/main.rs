use maelstrom::{EchoMsg, EchoOk, EchoOkBody, InitMsg, InitOk, InitOkBody, Node};
use serde_json::Result;

fn main() -> Result<()> {
    let init_msg_str = r#"
    {
        "src": "c1",
        "dest": "n3",
        "body": {
            "type": "init",
            "msg_id": 1,
            "node_id":  "n3",
            "node_ids": ["n1", "n2", "n3"]
        }
    }
    "#;
    let init_msg: InitMsg = serde_json::from_str(init_msg_str)?;
    println!("{:?}", init_msg);

    let node = Node {
        node_id: init_msg.body.node_id.clone(),
        node_ids: init_msg.body.node_ids.clone(),
    };

    let init_ok = InitOk {
        src: node.node_id.clone(),
        dest: init_msg.src.clone(),
        body: InitOkBody {
            msg_type: String::from("init_ok"),
            in_reply_to: init_msg.body.msg_id,
        },
    };
    println!("{:?}", init_ok);

    let data = r#"
    {
        "src": "c1",
        "dest": "n3",
        "body": {
            "type": "echo",
            "msg_id": 1,
            "echo": "Please echo 35"
        }
    }
    "#;
    let echo_msg: EchoMsg = serde_json::from_str(data)?;
    println!("{:?}", echo_msg);
    let echo_reply = EchoOk {
        src: node.node_id.clone(),
        dest: echo_msg.src.clone(),
        body: EchoOkBody {
            msg_type: String::from("echo_ok"),
            msg_id: echo_msg.body.msg_id,
            in_reply_to: echo_msg.body.msg_id,
            echo: echo_msg.body.echo,
        },
    };
    println!("{:?}", echo_reply);
    Ok(())
}
