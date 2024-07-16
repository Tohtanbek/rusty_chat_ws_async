use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use iced::{Application, Command, Element, Subscription, Theme};
use iced::widget::{button, column, Column, radio, text, text_input};
use iced::window::{close, Id};
use tokio::join;
use tokio::sync::{mpsc};
use tokio::sync::mpsc::Receiver;

use Message::*;

use crate::gui_user::{Screen, User};
use crate::ws_handler::{connect, UserAction};

pub struct App {
    screen: Screen,
    user: User,
    chosen_user: Option<String>,
    online_users: Arc<Mutex<HashSet<String>>>,
    prev_messages: Vec<String>,
    msg_input: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    Disconnection,
    LoginInputChanged(String),
    RadioButtonChanged(String),
    UserChosenButton,
    MsgInputChanged(String),
    SendMsgButton,
    Mock,
}

impl Application for App {
    type Executor = futures::executor::ThreadPool;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let (sender, receiver) = mpsc::channel(100);
        let users = Arc::new(Mutex::new(HashSet::new()));
        let app = Self {
            screen: Screen::Login,
            user: User::default(),
            chosen_user: None,
            online_users: users.clone(),
            prev_messages: Vec::new(),
            msg_input: "".to_string(),
        };
        let connection_command =
            Command::perform({
                                 let first_task = connect(sender);
                                 let second_task = update_online_users(users.clone(), receiver);
                                 tokio::spawn(async { join!(first_task, second_task) })
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
                self.chosen_user = Some(value);
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
                self.prev_messages.push(self.msg_input.clone());
                self.msg_input.clear();
                Command::none()
            }
            Mock => Command::none(),
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

        let chat_history = text(self.prev_messages.join("\n"));
        let chat_input = text_input("Type your msg", &self.msg_input)
            .on_input(MsgInputChanged)
            .padding(10);
        let send_msg_button = button("Send")
            .padding(10)
            .on_press(SendMsgButton);

        match self.screen {
            Screen::Login => {
                Column::new()
                    .push(login_input)
                    .push(start_chat_button)
                    .extend(self.manage_radio())
                    .into()
            }
            Screen::Chat => column![chat_history, chat_input, send_msg_button].into()
        }
    }
}

impl App {
    fn manage_radio(&self) -> Vec<Element<Message>> {
        let chosen_user = self.chosen_user.as_ref();
        let users = self.online_users.lock().unwrap();

        users.iter().map(|login| {
            radio(
                login,
                login,
                chosen_user,
                |value| RadioButtonChanged(value.clone()),
            ).into()
        }).collect()
    }
}

async fn update_online_users(users: Arc<Mutex<HashSet<String>>>, mut receiver: Receiver<UserAction>) {
    while let Some(value) = receiver.recv().await {
        if let UserAction::Connected(ip) = value {
            users.lock().unwrap().insert(ip);
        }
    }
}



