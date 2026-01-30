use game_engine::network;

fn main() {
    env_logger::init();
    log::info!("Starting server v{}...", game_engine::VERSION);
    log::info!("Listening on port {}", network::DEFAULT_PORT);
}
