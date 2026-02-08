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
}
