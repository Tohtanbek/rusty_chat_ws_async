use iced::{Application, Settings};

use ws_client::gui::App;

#[tokio::main]
async fn main() {
    App::run(Settings::default()).unwrap();
}


