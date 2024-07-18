use std::any::TypeId;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use iced::{Application, Command, Element, Subscription, Theme};
use iced::advanced::graphics::futures::BoxStream;
use iced::advanced::Hasher;
use iced::advanced::subscription::{EventStream, Recipe};
use iced::widget::{button, column, Column, radio, Row, text, text_input};
use iced::window::{close, Id};
use tokio::sync::{mpsc, Mutex};
use tokio::sync::mpsc::{Receiver, Sender};

use common_lib::{ChatMsg, UserAction};
use Message::*;
use UserAction::{Connected, Disconnected, ReceivedMsgInChat, RequestedIp, RequestedOnlineList, SentMsgInChat};

use crate::gui_user::{Screen, User};
use crate::ws_handler::connect;

pub struct App {
    screen: Screen,
    user: User,
    chosen_user: Option<SocketAddr>,
    online_users: Arc<std::sync::Mutex<HashSet<String>>>,
    prev_messages: HashMap<Box<str>, Vec<Box<str>>>,
    msg_input: String,
    receiver: Arc<Mutex<Receiver<UserAction>>>,
    sender_to_ws: Arc<Sender<UserAction>>
}

#[derive(Debug, Clone)]
pub enum Message {
    Disconnection,
    LoginInputChanged(String),
    RadioButtonChanged(String),
    UserChosenButton,
    MsgInputChanged(String),
    SendMsgButton,
    ToMainMenuButton,
    UserConnected(String),
    ReceivedIp(SocketAddr),
    ReceivedOnlineList(Vec<String>),
    ReceivedMsgInChat(ChatMsg),
    UserDisconnected(String),
    Mock
}

impl Application for App {
    type Executor = futures::executor::ThreadPool;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let users = Arc::new(std::sync::Mutex::new(HashSet::new()));

        let (sender_to_gui, receiver) = mpsc::channel(100);
        let receiver = Arc::new(Mutex::new(receiver));

        let (sender_to_ws, receiver_from_gui) = mpsc::channel(100);

        let app = Self {
            screen: Screen::Login,
            user: User::default(),
            chosen_user: None,
            online_users: users.clone(),
            prev_messages: HashMap::new(),
            msg_input: "".to_string(),
            receiver,
            sender_to_ws: Arc::new(sender_to_ws)
        };

        let first_task = connect(sender_to_gui, receiver_from_gui);
        let connection_command =
            Command::perform({
                                 tokio::spawn(first_task)
                             }, |_| Disconnection);
        (app, connection_command)
    }


    fn title(&self) -> String {
        "Chat with...".to_string()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Disconnection => {
                close(Id::MAIN)
            }
            LoginInputChanged(value) => {
                self.user.login = Some(value);
                Command::none()
            }
            RadioButtonChanged(value) => {
                self.chosen_user = Some(value.parse().unwrap());
                Command::none()
            }
            UserChosenButton => {
                self.screen = Screen::Chat;
                Command::none()
            }
            MsgInputChanged(value) => {
                self.msg_input = value;
                Command::none()
            }
            SendMsgButton => {
                let chat_msg =
                    ChatMsg::new(
                        self.user.ip.unwrap(),
                        self.chosen_user.unwrap(),
                        Box::from(self.msg_input.as_str())
                    );
                let sender = self.sender_to_ws.clone();
                self.update_msg_history();
                Command::perform(
                    async move {
                        sender.send(SentMsgInChat(chat_msg)).await.unwrap()
                    },
                    |_| Mock
                )
            }
            ToMainMenuButton => {
                self.screen = Screen::Login;
                Command::none()
            }
            UserConnected(ip) => {
                self.online_users.lock().unwrap().insert(ip.clone());
                println!("new online_user ip {ip} saved");
                Command::none()
            }
            ReceivedIp(ip) => {
                self.user.ip = Some(ip);
                Command::none()
            }
            Message::ReceivedMsgInChat(msg) => {
                let sender_ip_string = msg.sender_ip.to_string();
                let sender_ip = sender_ip_string.as_str();
                let message = format!("{}: {}", sender_ip_string, msg.msg);
                if let Some(chat) = self.prev_messages.get_mut(sender_ip) {
                    chat.push(Box::from(message))
                } else {
                    self.prev_messages.insert(Box::from(sender_ip),vec![Box::from(message)]);
                }
                Command::none()
            }
            ReceivedOnlineList(list) => {
                let mut db = self.online_users.lock().unwrap();
                for element in list {
                    db.insert(element);
                }
                Command::none()
            }
            UserDisconnected(ip) => {
                self.online_users.lock().unwrap().remove(&ip);
                println!("disconnected user ip {ip} deleted");
                Command::none()
            }
            Mock => Command::none()
        }
    }


    fn view(&self) -> Element<'_, Self::Message> {
        let login_input =
            text_input("Type your login", self.user.login.as_ref().unwrap_or(&String::from("")))
                .on_input(LoginInputChanged)
                .padding(10);

        let start_chat_button = button("Chat!")
            .on_press(UserChosenButton)
            .padding(10);

        let mut chat_history = text("");
        if let Some(user) = self.chosen_user.as_ref() {
            let chosen_user_msg_history =
                self.prev_messages.get(user.to_string().as_str());
            if let Some(list) = chosen_user_msg_history {
                chat_history = text(list.join("\n"));
            }
        }

        let chat_input = text_input("Type your msg", &self.msg_input)
            .on_input(MsgInputChanged)
            .padding(10);
        let send_msg_button = button("Send")
            .padding(10)
            .on_press(SendMsgButton);
        let main_menu_button = button("Main menu")
            .padding(10)
            .on_press(ToMainMenuButton);
        let bottom_row =
            Row::from_vec(vec![
                send_msg_button.into(),
                main_menu_button.into()]);

        match self.screen {
            Screen::Login => {
                Column::new()
                    .push(login_input)
                    .push(start_chat_button)
                    .extend(self.manage_radio())
                    .into()
            }
            Screen::Chat => column![chat_history, chat_input, bottom_row].into()
        }
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let receiver = self.receiver.clone();
        let subscription =
            UserSubscription { receiver };
        Subscription::from_recipe(subscription)
    }
}

struct UserSubscription {
    receiver: Arc<Mutex<Receiver<UserAction>>>,
}

impl Recipe for UserSubscription {
    type Output = Message;

    fn hash(&self, state: &mut Hasher) {
        TypeId::of::<Self>().hash(state);
    }

    fn stream(self: Box<Self>, _input: EventStream) -> BoxStream<Self::Output> {
        let receiver = self.receiver;
        Box::pin(futures::stream::unfold
            (receiver, |receiver| async move {
                let action = receiver.lock().await.recv().await;
                if let Some(value) = action {
                    match value {
                        Connected(value) => {
                            Some((UserConnected(value), receiver))
                        }
                        RequestedIp(ip) => {
                            Some((ReceivedIp(ip), receiver))
                        }
                        RequestedOnlineList(value) => {
                            Some((ReceivedOnlineList(value), receiver))
                        }
                        Disconnected(value) => {
                            Some((UserDisconnected(value), receiver))
                        }
                        ReceivedMsgInChat(msg) => {
                            Some((Message::ReceivedMsgInChat(msg),receiver))
                        }
                        _ => {None}
                    }
                } else {
                    println!("GUI subscription closed after disconnection");
                    None
                }
            }))
    }
}

impl App {
    fn manage_radio(&self) -> Vec<Element<Message>> {
        let mock = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);
        let chosen_user = self.chosen_user.unwrap_or(mock).to_string();
        let users = self.online_users.lock().unwrap();

        users.iter().map(|ip| {
            radio(
                ip,
                ip,
                Some(&chosen_user),
                |value| RadioButtonChanged(value.clone()),
            ).into()
        }).collect()
    }

    fn update_msg_history(&mut self) {
        let your_msg = format!("You: {}",self.msg_input.as_str());
        let your_boxed_msg = Box::from(your_msg);
        let other_user_ip = self.chosen_user.as_ref().unwrap();

        if let Some(msg_history) =
            self.prev_messages.get_mut(other_user_ip.to_string().as_str())
        {
            msg_history.push(your_boxed_msg);
        } else {
            let key = Box::from(other_user_ip.to_string().as_str());
            let value = vec![your_boxed_msg];
            self.prev_messages.insert(key, value);
        }

        self.msg_input.clear();
    }
}



