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
    pub biomes: Option<Vec<u8>>,
}

#[derive(Clone)]
pub struct PlayerPosition {
    pub position: Option<(f64, f64, f64)>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
    pub flags: Option<u8>,
    pub on_ground: Option<bool>,
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
    PlayerMove {
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
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
