use std::collections::VecDeque;

use bevy::{ecs::resource::Resource, prelude::Vec3};
use crossbeam::channel::{Receiver, Sender};
use rs_protocol::protocol::UUID;

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
    pub paused: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetEntityKind {
    Player,
}

#[derive(Debug, Clone)]
pub enum NetEntityMessage {
    LocalPlayerId {
        entity_id: i32,
    },
    PlayerInfoAdd {
        uuid: UUID,
        name: String,
    },
    PlayerInfoRemove {
        uuid: UUID,
    },
    Spawn {
        entity_id: i32,
        uuid: Option<UUID>,
        kind: NetEntityKind,
        pos: Vec3,
        yaw: f32,
        pitch: f32,
        on_ground: Option<bool>,
    },
    MoveDelta {
        entity_id: i32,
        delta: Vec3,
        on_ground: Option<bool>,
    },
    Look {
        entity_id: i32,
        yaw: f32,
        pitch: f32,
        on_ground: Option<bool>,
    },
    Teleport {
        entity_id: i32,
        pos: Vec3,
        yaw: f32,
        pitch: f32,
        on_ground: Option<bool>,
    },
    Destroy {
        entity_ids: Vec<i32>,
    },
}

#[derive(Resource)]
pub struct ToNet(pub Sender<ToNetMessage>);

#[derive(Resource)]
pub struct FromNet(pub Receiver<FromNetMessage>);

#[derive(Resource, Default)]
pub struct Chat(pub VecDeque<String>, pub String);

#[derive(Resource, Debug, Clone, Copy)]
pub struct PlayerStatus {
    pub health: f32,
    pub food: i32,
    pub food_saturation: f32,
    pub dead: bool,
}

impl Default for PlayerStatus {
    fn default() -> Self {
        Self {
            health: 20.0,
            food: 20,
            food_saturation: 5.0,
            dead: false,
        }
    }
}

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct PerfTimings {
    pub frame_delta_ms: f32,
    pub main_thread_ms: f32,
    pub update_ms: f32,
    pub post_update_ms: f32,
    pub fixed_update_ms: f32,
    pub handle_messages_ms: f32,
    pub input_collect_ms: f32,
    pub fixed_tick_ms: f32,
    pub net_apply_ms: f32,
    pub smoothing_ms: f32,
    pub apply_transform_ms: f32,
    pub debug_ui_ms: f32,
    pub ui_ms: f32,
}

pub enum ToNetMessage {
    Connect {
        username: String,
        address: String,
    },
    Disconnect,
    Shutdown,
    ChatMessage(String),
    Respawn,
    PlayerMove {
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    PlayerAction {
        action_id: i8,
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
    UpdateHealth {
        health: f32,
        food: i32,
        food_saturation: f32,
    },
    NetEntity(NetEntityMessage),
}
