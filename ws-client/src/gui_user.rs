use std::net::SocketAddr;

#[derive(Clone, Debug)]
pub struct User {
    pub login: Option<String>,
    pub ip: Option<SocketAddr>
}

impl Default for User {
    fn default() -> Self {
        User{
            login: None,
            ip: None
        }
    }
}

pub enum Screen {
    Login,
    Chat
}