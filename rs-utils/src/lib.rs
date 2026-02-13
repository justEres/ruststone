use std::collections::{HashMap, VecDeque};

use bevy::{ecs::resource::Resource, prelude::Vec3};
use crossbeam::channel::{Receiver, Sender};
use rs_protocol::protocol::UUID;

pub mod registry;
pub use registry::{
    BlockFace, BlockModelKind, TEXTUREPACK_BLOCKS_BASE, TEXTUREPACK_ITEMS_BASE, block_model_kind,
    block_name, block_registry_key, block_state_id, block_state_meta, block_texture_name,
    item_name, item_registry_key,
};

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
    pub inventory_open: bool,
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

#[derive(Debug, Clone, Copy)]
pub struct BlockUpdate {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block_id: u16,
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
    Item,
    ExperienceOrb,
    Mob(MobKind),
    Object(ObjectKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PlayerSkinModel {
    #[default]
    Classic,
    Slim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MobKind {
    Creeper,
    Skeleton,
    Spider,
    Giant,
    Zombie,
    Slime,
    Ghast,
    PigZombie,
    Enderman,
    CaveSpider,
    Silverfish,
    Blaze,
    MagmaCube,
    EnderDragon,
    Wither,
    Bat,
    Witch,
    Endermite,
    Guardian,
    Pig,
    Sheep,
    Cow,
    Chicken,
    Squid,
    Wolf,
    Mooshroom,
    SnowGolem,
    Ocelot,
    IronGolem,
    Horse,
    Rabbit,
    Villager,
    Unknown(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    Boat,
    Minecart,
    Arrow,
    Snowball,
    ItemFrame,
    LeashKnot,
    EnderPearl,
    EnderEye,
    Firework,
    LargeFireball,
    SmallFireball,
    WitherSkull,
    Egg,
    SplashPotion,
    ExpBottle,
    FishingHook,
    PrimedTnt,
    ArmorStand,
    EndCrystal,
    FallingBlock,
    Unknown(u8),
}

#[derive(Debug, Clone)]
pub enum NetEntityMessage {
    LocalPlayerId {
        entity_id: i32,
    },
    PlayerInfoAdd {
        uuid: UUID,
        name: String,
        skin_url: Option<String>,
        skin_model: PlayerSkinModel,
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
    Velocity {
        entity_id: i32,
        velocity: Vec3,
    },
    Pose {
        entity_id: i32,
        sneaking: bool,
    },
    Animation {
        entity_id: i32,
        animation: NetEntityAnimation,
    },
    SetLabel {
        entity_id: i32,
        label: String,
    },
    Destroy {
        entity_ids: Vec<i32>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetEntityAnimation {
    SwingMainArm,
    TakeDamage,
    LeaveBed,
    Unknown(u8),
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
    pub experience_bar: f32,
    pub level: i32,
    pub total_experience: i32,
    pub dead: bool,
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct BreakIndicator {
    pub active: bool,
    pub progress: f32,
    pub elapsed_secs: f32,
    pub total_secs: f32,
}

impl Default for BreakIndicator {
    fn default() -> Self {
        Self {
            active: false,
            progress: 0.0,
            elapsed_secs: 0.0,
            total_secs: 0.0,
        }
    }
}

impl Default for PlayerStatus {
    fn default() -> Self {
        Self {
            health: 20.0,
            food: 20,
            food_saturation: 5.0,
            experience_bar: 0.0,
            level: 0,
            total_experience: 0,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventoryItemStack {
    pub item_id: i32,
    pub count: u8,
    pub damage: i16,
}

#[derive(Debug, Clone)]
pub struct InventoryWindowInfo {
    pub id: u8,
    pub kind: String,
    pub title: String,
    pub slot_count: u8,
}

#[derive(Debug, Clone)]
pub enum InventoryMessage {
    WindowOpen(InventoryWindowInfo),
    WindowClose {
        id: u8,
    },
    WindowItems {
        id: u8,
        items: Vec<Option<InventoryItemStack>>,
    },
    WindowSetSlot {
        id: i8,
        slot: i16,
        item: Option<InventoryItemStack>,
    },
    ConfirmTransaction {
        id: u8,
        action_number: i16,
        accepted: bool,
    },
    SetCurrentHotbarSlot {
        slot: u8,
    },
}

#[derive(Resource, Debug, Default, Clone)]
pub struct InventoryState {
    pub player_slots: Vec<Option<InventoryItemStack>>,
    pub open_window: Option<InventoryWindowInfo>,
    pub window_slots: HashMap<u8, Vec<Option<InventoryItemStack>>>,
    pub cursor_item: Option<InventoryItemStack>,
    pub selected_hotbar_slot: u8,
    pub next_action_number: u16,
    pub pending_confirm_acks: Vec<(u8, i16)>,
}

impl InventoryState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn queue_confirm_ack(&mut self, id: u8, action_number: i16) {
        self.pending_confirm_acks.push((id, action_number));
    }

    pub fn drain_confirm_acks(&mut self) -> Vec<(u8, i16)> {
        std::mem::take(&mut self.pending_confirm_acks)
    }

    pub fn set_window_items(&mut self, id: u8, items: Vec<Option<InventoryItemStack>>) {
        if id == 0 {
            self.player_slots = items;
        } else {
            self.window_slots.insert(id, items);
        }
    }

    pub fn set_slot(&mut self, id: i8, slot: i16, item: Option<InventoryItemStack>) {
        if id == -1 && slot == -1 {
            self.cursor_item = item;
            return;
        }
        if slot < 0 {
            return;
        }
        let slot = slot as usize;
        if id == 0 {
            if self.player_slots.len() <= slot {
                self.player_slots.resize(slot + 1, None);
            }
            self.player_slots[slot] = item;
            return;
        }
        if id > 0 {
            let window_id = id as u8;
            let slots = self.window_slots.entry(window_id).or_default();
            if slots.len() <= slot {
                slots.resize(slot + 1, None);
            }
            slots[slot] = item;
        }
    }

    pub fn hotbar_slot_index(&self, hotbar_index: u8) -> Option<usize> {
        if hotbar_index > 8 {
            return None;
        }
        if self.player_slots.len() >= 45 {
            Some(36 + hotbar_index as usize)
        } else if self.player_slots.len() >= 9 {
            Some(self.player_slots.len() - 9 + hotbar_index as usize)
        } else {
            None
        }
    }

    pub fn hotbar_item(&self, hotbar_index: u8) -> Option<InventoryItemStack> {
        let idx = self.hotbar_slot_index(hotbar_index)?;
        self.player_slots.get(idx).copied().flatten()
    }

    pub fn apply_local_click_player_window(
        &mut self,
        slot: i16,
        button: u8,
        mode: u8,
    ) -> Option<InventoryItemStack> {
        match mode {
            0 => self.apply_mode_normal_click(slot, button),
            1 => self.apply_mode_shift_click(slot),
            2 => self.apply_mode_number_key(slot, button),
            4 => self.apply_mode_drop(slot, button),
            6 => self.apply_mode_double_click(slot, button),
            _ => self.cursor_item,
        }
    }

    fn apply_outside_click(&mut self, button: u8) {
        match self.cursor_item {
            Some(_) if button == 0 => {
                self.cursor_item = None;
            }
            Some(mut cursor) if button == 1 => {
                cursor.count = cursor.count.saturating_sub(1);
                self.cursor_item = if cursor.count == 0 {
                    None
                } else {
                    Some(cursor)
                };
            }
            _ => {}
        }
    }

    fn apply_mode_normal_click(&mut self, slot: i16, button: u8) -> Option<InventoryItemStack> {
        if slot < 0 {
            self.apply_outside_click(button);
            return self.cursor_item;
        }

        let slot_index = slot as usize;
        if self.player_slots.len() <= slot_index {
            self.player_slots.resize(slot_index + 1, None);
        }

        let mut slot_item = self.player_slots[slot_index];
        let mut cursor = self.cursor_item;

        if button == 0 {
            match (cursor, slot_item) {
                (None, Some(_)) => {
                    cursor = slot_item;
                    slot_item = None;
                }
                (Some(_), None) => {
                    slot_item = cursor;
                    cursor = None;
                }
                (Some(cur), Some(mut sl)) => {
                    if can_stack(cur, sl) && sl.count < max_stack_for_item(sl.item_id) {
                        let max = max_stack_for_item(sl.item_id);
                        let space = max.saturating_sub(sl.count);
                        let moved = space.min(cur.count);
                        sl.count = sl.count.saturating_add(moved);
                        let remaining = cur.count.saturating_sub(moved);
                        slot_item = Some(sl);
                        cursor = if remaining == 0 {
                            None
                        } else {
                            Some(InventoryItemStack {
                                count: remaining,
                                ..cur
                            })
                        };
                    } else {
                        slot_item = Some(cur);
                        cursor = Some(sl);
                    }
                }
                _ => {}
            }
        } else if button == 1 {
            match (cursor, slot_item) {
                (None, Some(mut sl)) => {
                    let take = (sl.count.saturating_add(1)) / 2;
                    let remain = sl.count.saturating_sub(take);
                    cursor = Some(InventoryItemStack { count: take, ..sl });
                    slot_item = if remain == 0 {
                        None
                    } else {
                        sl.count = remain;
                        Some(sl)
                    };
                }
                (Some(mut cur), None) => {
                    slot_item = Some(InventoryItemStack { count: 1, ..cur });
                    cur.count = cur.count.saturating_sub(1);
                    cursor = if cur.count == 0 { None } else { Some(cur) };
                }
                (Some(mut cur), Some(mut sl)) => {
                    if can_stack(cur, sl) && sl.count < max_stack_for_item(sl.item_id) {
                        sl.count = sl.count.saturating_add(1);
                        cur.count = cur.count.saturating_sub(1);
                        slot_item = Some(sl);
                        cursor = if cur.count == 0 { None } else { Some(cur) };
                    } else {
                        slot_item = Some(cur);
                        cursor = Some(sl);
                    }
                }
                _ => {}
            }
        }

        self.player_slots[slot_index] = slot_item;
        self.cursor_item = cursor;
        self.cursor_item
    }

    fn apply_mode_shift_click(&mut self, slot: i16) -> Option<InventoryItemStack> {
        if slot < 0 {
            return self.cursor_item;
        }
        let slot_index = slot as usize;
        if slot_index >= self.player_slots.len() {
            return self.cursor_item;
        }
        let Some(mut moving) = self.player_slots[slot_index].take() else {
            return self.cursor_item;
        };

        let target_range = if (9..=35).contains(&slot_index) {
            36..45
        } else if (36..=44).contains(&slot_index) {
            9..36
        } else {
            self.player_slots[slot_index] = Some(moving);
            return self.cursor_item;
        };

        for idx in target_range.clone() {
            if moving.count == 0 {
                break;
            }
            let Some(existing) = self.player_slots.get_mut(idx) else {
                continue;
            };
            if let Some(stack) = existing.as_mut() {
                if can_stack(*stack, moving) && stack.count < max_stack_for_item(stack.item_id) {
                    let max = max_stack_for_item(stack.item_id);
                    let space = max.saturating_sub(stack.count);
                    let moved = space.min(moving.count);
                    stack.count = stack.count.saturating_add(moved);
                    moving.count = moving.count.saturating_sub(moved);
                }
            }
        }

        for idx in target_range {
            if moving.count == 0 {
                break;
            }
            let Some(existing) = self.player_slots.get_mut(idx) else {
                continue;
            };
            if existing.is_none() {
                *existing = Some(moving);
                moving.count = 0;
            }
        }

        if moving.count > 0 {
            self.player_slots[slot_index] = Some(moving);
        }
        self.cursor_item
    }

    fn apply_mode_number_key(&mut self, slot: i16, button: u8) -> Option<InventoryItemStack> {
        if slot < 0 || button > 8 {
            return self.cursor_item;
        }
        let slot_index = slot as usize;
        let hotbar_index = 36 + button as usize;
        if slot_index >= self.player_slots.len() || hotbar_index >= self.player_slots.len() {
            return self.cursor_item;
        }
        self.player_slots.swap(slot_index, hotbar_index);
        self.cursor_item
    }

    fn apply_mode_drop(&mut self, slot: i16, button: u8) -> Option<InventoryItemStack> {
        if slot < 0 {
            return self.cursor_item;
        }
        let slot_index = slot as usize;
        if slot_index >= self.player_slots.len() {
            return self.cursor_item;
        }
        if let Some(mut stack) = self.player_slots[slot_index] {
            if button == 0 {
                stack.count = stack.count.saturating_sub(1);
                self.player_slots[slot_index] = if stack.count == 0 { None } else { Some(stack) };
            } else {
                self.player_slots[slot_index] = None;
            }
        }
        self.cursor_item
    }

    fn apply_mode_double_click(&mut self, slot: i16, button: u8) -> Option<InventoryItemStack> {
        if button != 0 {
            return self.cursor_item;
        }
        let mut cursor = match self.cursor_item {
            Some(c) => c,
            None => return None,
        };
        let max = max_stack_for_item(cursor.item_id);
        if cursor.count >= max {
            return self.cursor_item;
        }

        let skip_slot = if slot >= 0 { Some(slot as usize) } else { None };
        for idx in 0..self.player_slots.len() {
            if Some(idx) == skip_slot {
                continue;
            }
            let Some(mut stack) = self.player_slots[idx] else {
                continue;
            };
            if !can_stack(stack, cursor) {
                continue;
            }
            let need = max.saturating_sub(cursor.count);
            if need == 0 {
                break;
            }
            let moved = need.min(stack.count);
            stack.count = stack.count.saturating_sub(moved);
            cursor.count = cursor.count.saturating_add(moved);
            self.player_slots[idx] = if stack.count == 0 { None } else { Some(stack) };
            if cursor.count >= max {
                break;
            }
        }
        self.cursor_item = Some(cursor);
        self.cursor_item
    }
}

fn can_stack(a: InventoryItemStack, b: InventoryItemStack) -> bool {
    a.item_id == b.item_id && a.damage == b.damage
}

fn max_stack_for_item(item_id: i32) -> u8 {
    if is_single_stack_item(item_id) { 1 } else { 64 }
}

fn is_single_stack_item(item_id: i32) -> bool {
    matches!(
        item_id,
        256..=259
            | 261
            | 267..=279
            | 283..=286
            | 290..=294
            | 298..=317
            | 326..=330
            | 346
            | 359
    )
}

pub enum ToNetMessage {
    Connect {
        username: String,
        address: String,
        auth_mode: AuthMode,
        auth_account_uuid: Option<String>,
        prism_accounts_path: Option<String>,
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

use rs_protocol::protocol::packet::Packet;

pub enum FromNetMessage {
    Connected,
    Disconnected,
    Packet(Packet),
    ChatMessage(String),
    ChunkData(ChunkData),
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
    Inventory(InventoryMessage),
    NetEntity(NetEntityMessage),
}
