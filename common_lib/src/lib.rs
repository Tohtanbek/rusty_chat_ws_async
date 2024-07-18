use std::net::SocketAddr;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum UserAction {
    Connected(String),
    RequestedOnlineList(Vec<String>),
    RequestedIp(SocketAddr),
    Disconnected(String),
    SentMsgInChat(ChatMsg),
    ReceivedMsgInChat(ChatMsg)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatMsg {
    pub sender_ip: SocketAddr,
    pub receiver_ip: SocketAddr,
    pub msg: Box<str>,
}

impl ChatMsg {
    pub fn new(sender_ip: SocketAddr, receiver_ip: SocketAddr, msg: Box<str>) -> Self {
        ChatMsg { sender_ip, receiver_ip, msg }
    }
}

pub fn deserialize_user_action(vec: Vec<u8>) -> Result<UserAction, bincode::Error> {
    bincode::deserialize(vec.as_slice())
}

pub fn serialize_user_action(data: UserAction) -> Vec<u8> {
    bincode::serialize(&data).unwrap()
}