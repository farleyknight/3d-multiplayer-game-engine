use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};

use game_engine::network::{
    deserialize_client_packet, serialize_server_packet, ClientPacket, ConnectedClient,
    ServerPacket, DEFAULT_PORT,
};
use game_engine::types::PlayerData;

fn main() {
    env_logger::init();
    log::info!("Starting server v{}...", game_engine::VERSION);

    let bind_addr = format!("0.0.0.0:{}", DEFAULT_PORT);
    let socket = match UdpSocket::bind(&bind_addr) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to bind to {}: {}", bind_addr, e);
            return;
        }
    };

    socket
        .set_nonblocking(true)
        .expect("Failed to set non-blocking mode");

    log::info!("Listening on 0.0.0.0:{}", DEFAULT_PORT);

    let mut clients: HashMap<SocketAddr, ConnectedClient> = HashMap::new();
    let mut next_player_id: u32 = 1;
    let mut recv_buf = [0u8; 1024];

    loop {
        match socket.recv_from(&mut recv_buf) {
            Ok((len, src_addr)) => {
                handle_packet(
                    &socket,
                    &recv_buf[..len],
                    src_addr,
                    &mut clients,
                    &mut next_player_id,
                );
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                // No data available, continue loop
            }
            Err(e) => {
                log::error!("recv_from error: {}", e);
            }
        }

        // Check for timed out clients
        let timed_out: Vec<SocketAddr> = clients
            .iter()
            .filter(|(_, client)| client.is_timed_out())
            .map(|(addr, _)| *addr)
            .collect();

        for addr in timed_out {
            if let Some(client) = clients.remove(&addr) {
                log::info!("Client {} (player {}) timed out", addr, client.player_id);
                broadcast_player_left(&socket, &clients, client.player_id);
            }
        }

        // Brief sleep to avoid busy loop
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

fn handle_packet(
    socket: &UdpSocket,
    data: &[u8],
    src_addr: SocketAddr,
    clients: &mut HashMap<SocketAddr, ConnectedClient>,
    next_player_id: &mut u32,
) {
    let packet = match deserialize_client_packet(data) {
        Ok(p) => p,
        Err(e) => {
            log::warn!("Failed to deserialize packet from {}: {}", src_addr, e);
            return;
        }
    };

    match packet {
        ClientPacket::PlayerUpdate(state) => {
            if state.player_id == 0 {
                // New client requesting an ID
                let assigned_id = *next_player_id;
                *next_player_id += 1;

                let mut client = ConnectedClient::new(assigned_id);
                client.state.position = state.position;
                client.state.rotation_yaw = state.rotation_yaw;
                clients.insert(src_addr, client);

                log::info!("New client {} assigned player_id {}", src_addr, assigned_id);

                let welcome = ServerPacket::Welcome {
                    assigned_player_id: assigned_id,
                };
                send_packet(socket, &welcome, src_addr);
            } else if let Some(client) = clients.get_mut(&src_addr) {
                // Existing client update
                client.touch();
                client.state.position = state.position;
                client.state.rotation_yaw = state.rotation_yaw;
            } else {
                log::warn!(
                    "Unknown client {} sent player_id {}",
                    src_addr,
                    state.player_id
                );
            }
        }
    }

    // Broadcast world state to all clients
    broadcast_world_state(socket, clients);
}

fn send_packet(socket: &UdpSocket, packet: &ServerPacket, addr: SocketAddr) {
    match serialize_server_packet(packet) {
        Ok(bytes) => {
            if let Err(e) = socket.send_to(&bytes, addr) {
                log::error!("Failed to send packet to {}: {}", addr, e);
            }
        }
        Err(e) => {
            log::error!("Failed to serialize packet: {}", e);
        }
    }
}

fn broadcast_world_state(socket: &UdpSocket, clients: &HashMap<SocketAddr, ConnectedClient>) {
    let players: Vec<PlayerData> = clients.values().map(|c| c.state.into()).collect();

    let packet = ServerPacket::WorldState { players };

    for addr in clients.keys() {
        send_packet(socket, &packet, *addr);
    }
}

fn broadcast_player_left(
    socket: &UdpSocket,
    clients: &HashMap<SocketAddr, ConnectedClient>,
    player_id: u32,
) {
    let packet = ServerPacket::PlayerLeft { player_id };

    for addr in clients.keys() {
        send_packet(socket, &packet, *addr);
    }
}
