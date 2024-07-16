use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::Message;

#[derive(Serialize, Deserialize, Debug)]
pub enum UserAction {
    Connected(String),
    Disconnected(String),
}

pub async fn run_server() {
    let db: HashSet<SocketAddr> = HashSet::new();
    let db = Arc::new(Mutex::new(db));
    let listener = TcpListener::bind("127.0.0.1:1234").await.unwrap();
    loop {
        let (stream, address) = listener.accept().await.unwrap();
        println!("Connection attempt from {:?}", address.ip());
        tokio::spawn(handle_connection(stream, db.clone()));
    }
}

pub async fn handle_connection(stream: TcpStream, db: Arc<Mutex<HashSet<SocketAddr>>>) {
    let callback = |req: &Request, mut response: Response| {
        let headers = response.headers_mut();
        headers.append("Sec-WebSocket-Protocol", "rust_websocket".parse().unwrap());
        Ok(response)
    };

    let ip = stream.peer_addr().unwrap();
    let ws_stream =
        tokio_tungstenite::accept_hdr_async(stream, callback).await.unwrap();
    println!("Successful ws connection of {:?}", ip);
    db.lock().await.insert(ip);
    println!("actual ips: {:?}", db.lock().await);
    let (mut sink, mut stream) =
        ws_stream.split();

    let online_user_msg = Message::Binary(
        serialize_enum(UserAction::Connected(ip.to_string()))
    );
    sink.send(online_user_msg).await.unwrap();

    while let Some(message) = stream.next().await {
        let message = message.unwrap();
        match message {
            Message::Close(_) => {
                println!("Client disconnected {:?}", ip);
                db.lock().await.remove(&ip);
                println!("actual ips: {:?}", db.lock().await);
            }
            _ => sink.send(message).await.unwrap()
        }
    }
}

fn serialize_enum(data: UserAction) -> Vec<u8> {
    bincode::serialize(&data).unwrap()
}

