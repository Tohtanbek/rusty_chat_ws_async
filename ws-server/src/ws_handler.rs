use std::net::SocketAddr;
use std::sync::Arc;

use futures::{SinkExt, StreamExt};
use futures::stream::SplitSink;
use Message::{Binary, Close};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::handshake::server::{Request, Response};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use common_lib::{deserialize_user_action, serialize_user_action, UserAction};
use common_lib::UserAction::SentMsgInChat;
use UserAction::{Disconnected, ReceivedMsgInChat};

pub struct OnlineUser {
    ip: SocketAddr,
    sink: SplitSink<WebSocketStream<TcpStream>, Message>,
}

pub async fn run_server() {
    let db: Vec<OnlineUser> = Vec::new();
    let db = Arc::new(Mutex::new(db));
    let listener = TcpListener::bind("127.0.0.1:1234").await.unwrap();
    loop {
        let (stream, address) = listener.accept().await.unwrap();
        println!("Connection attempt from {:?}", address.ip());
        tokio::spawn(handle_connection(stream, db.clone()));
    }
}

async fn handle_connection(stream: TcpStream, mut db: Arc<Mutex<Vec<OnlineUser>>>) {
    let (ip, ws_stream) = connect_ws(stream).await;
    let (sink, mut stream) =
        ws_stream.split();

    handle_connected_user(&mut db, ip, sink).await;

    while let Some(message) = stream.next().await {
        let message = message.unwrap();
        match message {
            Close(_) => {
                println!("Client disconnected {:?}", ip);
                let mut users = db.lock().await;
                users.retain(|user| user.ip != ip);
                println!("online ips: {:?}", users.len());
                let offline_user_msg = Binary(
                    serialize_user_action(Disconnected(ip.to_string()))
                );
                for user in users.iter_mut() {
                    user.sink.send(offline_user_msg.clone()).await.unwrap()
                };
            }
            Binary(bytes) => {
                let action = deserialize_user_action(bytes).unwrap();
                match action {
                    SentMsgInChat(chat_msg) => {
                        println!("server starts processing new msg: {:?}", chat_msg);
                        if let Some(recipient) =
                            db.lock().await.iter_mut().find(|user| user.ip == chat_msg.receiver_ip) {
                            let msg = Binary(serialize_user_action(ReceivedMsgInChat(chat_msg)));
                            recipient.sink.send(msg).await.unwrap();
                            println!("msg sent to {}", recipient.ip)
                        }
                    }
                    _ => {}
                }
            }
            _ => {
                send_msg_by_user_id(&mut db, ip, message).await;
            }
        }
    }
}

async fn handle_connected_user(
    mut db: &mut Arc<Mutex<Vec<OnlineUser>>>,
    ip: SocketAddr,
    sink: SplitSink<WebSocketStream<TcpStream>, Message>,
) {
    let mut fresh_user = OnlineUser { ip, sink };
    send_ip_to_gui(ip, &mut fresh_user.sink).await;
    println!("sent ip {} to gui", ip);
    db.lock().await.push(fresh_user);
    println!("online ips: {:?}", db.lock().await.len());
    send_online_users_list(&mut db, ip).await;
    println!("List of online users sent successfully to {:?}", ip);

    send_out_fresh_user(&mut db, ip).await;
}

async fn send_ip_to_gui(ip: SocketAddr, sink: &mut SplitSink<WebSocketStream<TcpStream>, Message>) {
    sink.send(Binary(
        serialize_user_action(
            UserAction::RequestedIp(ip)))
    ).await.unwrap();
}

async fn connect_ws(stream: TcpStream) -> (SocketAddr, WebSocketStream<TcpStream>) {
    let callback = |req: &Request, mut response: Response| {
        let headers = response.headers_mut();
        headers.append("Sec-WebSocket-Protocol", "rust_websocket".parse().unwrap());
        Ok(response)
    };
    let ip = stream.peer_addr().unwrap();
    let ws_stream =
        tokio_tungstenite::accept_hdr_async(stream, callback).await.unwrap();
    println!("Successful ws connection of {:?}", ip);
    (ip, ws_stream)
}

async fn send_out_fresh_user(db: &mut Arc<Mutex<Vec<OnlineUser>>>, ip: SocketAddr) {
    let online_user_msg = Binary(
        serialize_user_action(UserAction::Connected(ip.to_string()))
    );
    for user in
        db.lock().await.iter_mut().filter(|user| user.ip != ip)
    {
        user.sink.send(online_user_msg.clone()).await.unwrap()
    }
}

async fn send_online_users_list(db: &mut Arc<Mutex<Vec<OnlineUser>>>, ip: SocketAddr) {
    let list = db.lock().await.iter()
        .filter(|user| user.ip != ip)
        .map(|user| user.ip.to_string())
        .collect();
    let online_users_list_msg = Binary(
        serialize_user_action(UserAction::RequestedOnlineList(list))
    );

    send_msg_by_user_id(db, ip, online_users_list_msg).await;
}

async fn send_msg_by_user_id(
    db: &mut Arc<Mutex<Vec<OnlineUser>>>,
    ip: SocketAddr,
    online_users_list_msg: Message,
) {
    db.lock().await
        .iter_mut()
        .find(|user| user.ip == ip).unwrap()
        .sink.send(online_users_list_msg).await.unwrap()
}


