use bevy::ecs::resource::Resource;
use crossbeam::channel::{Receiver, Sender};
use rs_protocol::protocol::packet::Packet;

use crate::chat::TitleMessage;
use crate::entities::NetEntityMessage;
use crate::inventory::{InventoryItemStack, InventoryMessage};
use crate::scoreboard::ScoreboardMessage;
use crate::sound::SoundEvent;
use crate::world::{BlockUpdate, ChunkData, PlayerPosition};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuthMode {
    #[default]
    Offline,
    Authenticated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityUseAction {
    Interact,
    Attack,
}

pub enum ToNetMessage {
    Connect {
        username: String,
        address: String,
        auth_mode: AuthMode,
        auth_account_uuid: Option<String>,
        prism_accounts_path: Option<String>,
        requested_view_distance: u8,
    },
    Disconnect,
    Shutdown,
    ChatMessage(String),
    TabCompleteRequest {
        text: String,
    },
    Respawn,
    PlayerMovePosLook {
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    PlayerMovePos {
        x: f64,
        y: f64,
        z: f64,
        on_ground: bool,
    },
    PlayerMoveLook {
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    PlayerMoveGround {
        on_ground: bool,
    },
    PlayerAction {
        entity_id: i32,
        action_id: i8,
    },
    ClientAbilities {
        flags: u8,
        flying_speed: f32,
        walking_speed: f32,
    },
    SwingArm,
    UseEntity {
        target_id: i32,
        action: EntityUseAction,
    },
    HeldItemChange {
        slot: i16,
    },
    ClickWindow {
        id: u8,
        slot: i16,
        button: u8,
        mode: u8,
        action_number: u16,
        clicked_item: Option<InventoryItemStack>,
    },
    ConfirmTransaction {
        id: u8,
        action_number: i16,
        accepted: bool,
    },
    CloseWindow {
        id: u8,
    },
    DigStart {
        x: i32,
        y: i32,
        z: i32,
        face: u8,
    },
    DigCancel {
        x: i32,
        y: i32,
        z: i32,
        face: u8,
    },
    DigFinish {
        x: i32,
        y: i32,
        z: i32,
        face: u8,
    },
    PlaceBlock {
        x: i32,
        y: i32,
        z: i32,
        face: i8,
        cursor_x: u8,
        cursor_y: u8,
        cursor_z: u8,
    },
    UseItem {
        held_item: Option<InventoryItemStack>,
    },
    DropHeldItem {
        full_stack: bool,
    },
}

pub enum FromNetMessage {
    Connected,
    Disconnected,
    DisconnectReason(String),
    Packet(Packet),
    ChatMessage(String),
    TabCompleteReply(Vec<String>),
    Respawn,
    ChunkData(ChunkData),
    ChunkUnload {
        x: i32,
        z: i32,
    },
    BlockUpdates(Vec<BlockUpdate>),
    PlayerPosition(PlayerPosition),
    UpdateHealth {
        health: f32,
        food: i32,
        food_saturation: f32,
    },
    UpdateExperience {
        experience_bar: f32,
        level: i32,
        total_experience: i32,
    },
    GameMode {
        gamemode: u8,
    },
    TimeUpdate {
        world_age: i64,
        time_of_day: i64,
    },
    PlayerAbilities {
        flags: u8,
        flying_speed: f32,
        walking_speed: f32,
    },
    PotionEffect {
        entity_id: i32,
        effect_id: i8,
        amplifier: i8,
        duration_ticks: i32,
    },
    PotionEffectRemove {
        entity_id: i32,
        effect_id: i8,
    },
    Inventory(InventoryMessage),
    Sound(SoundEvent),
    Title(TitleMessage),
    TabListHeaderFooter {
        header: String,
        footer: String,
    },
    Scoreboard(ScoreboardMessage),
    NetEntity(NetEntityMessage),
}

#[derive(Resource)]
pub struct ToNet(pub Sender<ToNetMessage>);

#[derive(Resource)]
pub struct FromNet(pub Receiver<FromNetMessage>);
