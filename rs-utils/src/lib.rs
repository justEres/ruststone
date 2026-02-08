use std::collections::VecDeque;

use bevy::ecs::resource::Resource;
use crossbeam::channel::{Receiver, Sender};

#[derive(Resource)]
pub struct AppState(pub ApplicationState);

#[derive(Debug)]
pub enum ApplicationState {
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Resource, Default)]
pub struct UiState {
    pub chat_open: bool,
}

#[derive(Clone)]
pub struct ChunkSection {
    pub y: u8,
    pub blocks: Vec<u16>,
}

#[derive(Clone)]
pub struct ChunkData {
    pub x: i32,
    pub z: i32,
    pub full: bool,
    pub sections: Vec<ChunkSection>,
}

#[derive(Clone)]
pub struct PlayerPosition {
    pub position: Option<(f64, f64, f64)>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
    pub flags: Option<u8>,
}

#[derive(Resource)]
pub struct ToNet(pub Sender<ToNetMessage>);

#[derive(Resource)]
pub struct FromNet(pub Receiver<FromNetMessage>);

#[derive(Resource, Default)]
pub struct Chat(pub VecDeque<String>, pub String);

pub enum ToNetMessage {
    Connect { username: String, address: String },
    Disconnect,
    Shutdown,
    ChatMessage(String),
}

use rs_protocol::protocol::packet::Packet;

pub enum FromNetMessage {
    Connected,
    Disconnected,
    Packet(Packet),
    ChatMessage(String),
    ChunkData(ChunkData),
    PlayerPosition(PlayerPosition),
}
