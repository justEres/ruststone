use bevy::ecs::resource::Resource;
use crossbeam::channel::{Receiver, Sender};

#[derive(Resource)]
pub struct AppState(pub ApplicationState);

pub enum ApplicationState {
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Resource)]
pub struct ToNet(pub Sender<ToNetMessage>);

#[derive(Resource)]
pub struct FromNet(pub Receiver<FromNetMessage>);

pub enum ToNetMessage {
    Connect { username: String, address: String },
    Disconnect,
    Shutdown,
}

use rs_protocol::protocol::packet::Packet;

pub enum FromNetMessage {
    Connected,
    Disconnected,
    Packet(Packet),
}
