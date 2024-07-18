use futures::{SinkExt, StreamExt};
use futures::stream::{SplitSink, SplitStream};
use Message::{Binary, Text};
use tokio::io::{AsyncBufReadExt, BufReader, stdin};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tokio_tungstenite::tungstenite::{ClientRequestBuilder, Message};
use tokio_tungstenite::tungstenite::http::Uri;
use common_lib::{deserialize_user_action, serialize_user_action, UserAction};

pub async fn connect(
    sender_to_gui: Sender<UserAction>,
    receiver_from_gui: Receiver<UserAction>,
) {
    let builder = ClientRequestBuilder::new(Uri::from_static("ws://127.0.0.1:1234"))
        .with_sub_protocol("rust_websocket");
    let (connection, _) = connect_async(builder).await.unwrap();

    let (mut sink, stream)
        = connection.split();
    sink.send(Text("Started main ws logic".to_string())).await.unwrap();

    tokio::select! {
        _ = handle_server_input(stream, sender_to_gui) => {
            sink.send(Message::Close(None)).await.unwrap()
        }
        _ = handle_cmd_input() => {
            sink.send(Message::Close(None)).await.unwrap()
        }
        _ = handle_gui_input(receiver_from_gui, &mut sink) => {

        }
    }
}

async fn handle_gui_input(
    mut receiver: Receiver<UserAction>,
    sink: &mut SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>
) {
    while let Some(user_action) = receiver.recv().await {
        sink.send(Binary(serialize_user_action(user_action))).await.unwrap();
        println!("Sent user action from client to ws server")
    }
}

async fn handle_server_input(
    stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    sender_to_gui: Sender<UserAction>,
) {
    stream.for_each(|msg| async {
        match msg {
            Ok(Binary(data)) => {
                println!("Received binary data: {:?}", data);
                match deserialize_user_action(data) {
                    Ok(user_action) => {
                        if let Err(e) = sender_to_gui.send(user_action).await {
                            eprintln!("Failed to send user action: {:?}", e);
                        }
                    }
                    Err(e) => eprintln!("Failed to deserialize: {:?}", e),
                }
            }
            Ok(Text(text)) => {
                println!("Received text message: {}", text);
            }
            Ok(_) => println!("Received other type of message"),
            Err(e) => eprintln!("Error receiving message: {:?}", e),
        }
    }).await;
}

async fn handle_cmd_input() {
    let stdin = stdin();
    let mut cli_stream = BufReader::new(stdin).lines();
    while let Some(command) = cli_stream.next_line().await.unwrap() {
        match command.as_str() {
            "stop" => return,
            _ => println!("Command doesnt exist")
        }
    }
}

