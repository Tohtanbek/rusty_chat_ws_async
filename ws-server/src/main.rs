use ws_server::ws_handler::run_server;

#[tokio::main]
async fn main() {
    run_server().await
}
