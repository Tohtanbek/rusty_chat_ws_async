use std::future::Future;

use futures::{SinkExt, StreamExt};
use futures::stream::SplitStream;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader, stdin};
use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tokio_tungstenite::tungstenite::{ClientRequestBuilder, Message};
use tokio_tungstenite::tungstenite::http::Uri;

#[derive(Serialize, Deserialize, Debug)]
pub enum UserAction {
    Connected(String),
    Disconnected(String),
}

pub async fn connect(sender: Sender<UserAction>) {
    let builder = ClientRequestBuilder::new(Uri::from_static("ws://127.0.0.1:1234"))
        .with_sub_protocol("rust_websocket");
    let (connection, _) = connect_async(builder).await.unwrap();

    let (mut sink, stream)
        = connection.split();
    sink.send(Message::Text("echo".to_string())).await.unwrap();

    tokio::select! {
        _ = handle_input(stream, sender) => {
            sink.send(Message::Close(None)).await.unwrap()
        }
        _ = wait_disconnect() => {sink.send(Message::Close(None)).await.unwrap()}
    }
}

async fn handle_input(
    stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    sender: Sender<UserAction>,
) {
    stream.for_each(|msg| async {
        match msg {
            Ok(Message::Binary(data)) => {
                println!("Received binary data: {:?}", data);
                match deserialize_vec(data) {
                    Ok(user_action) => {
                        if let Err(e) = sender.send(user_action).await {
                            eprintln!("Failed to send user action: {:?}", e);
                        }
                    },
                    Err(e) => eprintln!("Failed to deserialize: {:?}", e),
                }
            },
            Ok(Message::Text(text)) => {
                println!("Received unexpected text message: {}", text);
            },
            Ok(_) => println!("Received other type of message"),
            Err(e) => eprintln!("Error receiving message: {:?}", e),
        }
    }).await;
}

async fn wait_disconnect() {
    let stdin = stdin();
    let mut cli_stream = BufReader::new(stdin).lines();
    while let Some(command) = cli_stream.next_line().await.unwrap() {
        match command.as_str() {
            "stop" => return,
            _ => println!("Command doesnt exist")
        }
    }
}

fn deserialize_vec(vec: Vec<u8>) -> Result<UserAction, bincode::Error> {
    bincode::deserialize(vec.as_slice())
}