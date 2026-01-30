use game_engine::{network, render};

fn main() {
    env_logger::init();
    log::info!("Starting client v{}...", game_engine::VERSION);
    log::info!("Window title: {}", render::WINDOW_TITLE);
    log::info!("Connecting to server on port {}", network::DEFAULT_PORT);
}
