use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::time::Instant;

use bevy::{ecs::resource::Resource, prelude::Vec3};
use crossbeam::channel::{Receiver, Sender};
use rs_protocol::protocol::UUID;
use serde::{Deserialize, Serialize};

pub mod item_textures;
pub mod registry;
pub use item_textures::item_texture_candidates;
pub use registry::{
    BlockFace, BlockModelKind, TEXTUREPACK_BLOCKS_BASE, TEXTUREPACK_ITEMS_BASE, block_model_kind,
    block_name, block_registry_key, block_state_id, block_state_meta, block_texture_name,
    item_name, item_registry_key,
};

pub const RUSTSTONE_ASSETS_ROOT_ENV: &str = "RUSTSTONE_ASSETS_ROOT";

#[derive(Resource)]
pub struct AppState(pub ApplicationState);

pub fn ruststone_assets_root() -> PathBuf {
    if let Ok(explicit) = std::env::var(RUSTSTONE_ASSETS_ROOT_ENV) {
        let path = PathBuf::from(explicit);
        if path.exists() {
            return path;
        }
    }

    if let Ok(exe) = std::env::current_exe()
        && let Some(exe_dir) = exe.parent()
    {
        let sibling_assets = exe_dir.join("assets");
        if sibling_assets.exists() {
            return sibling_assets;
        }
    }

    let repo_assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../rs-client/assets");
    if repo_assets.exists() {
        return repo_assets;
    }

    PathBuf::from("assets")
}

pub fn texturepack_minecraft_root() -> PathBuf {
    ruststone_assets_root().join("texturepack/assets/minecraft")
}

pub fn texturepack_textures_root() -> PathBuf {
    texturepack_minecraft_root().join("textures")
}

pub fn sound_cache_root() -> PathBuf {
    PathBuf::from("ruststone_sound_cache")
}

pub fn sound_cache_minecraft_root() -> PathBuf {
    sound_cache_root().join("assets/minecraft")
}

#[derive(Debug, Clone, Copy)]
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
    pub ui_hidden: bool,
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct WorldTime {
    pub world_age: i64,
    pub time_of_day: i64,
    pub last_sync_instant: Option<Instant>,
}

impl Default for WorldTime {
    fn default() -> Self {
        Self {
            world_age: 0,
            time_of_day: 0,
            last_sync_instant: None,
        }
    }
}

impl WorldTime {
    pub fn interpolated_time_of_day(self, now: Instant) -> f32 {
        let fixed_time = self.time_of_day < 0;
        let base = if fixed_time {
            (-self.time_of_day) as f32
        } else {
            self.time_of_day as f32
        };

        if fixed_time {
            base
        } else {
            let elapsed_ticks = self
                .last_sync_instant
                .map(|instant| now.saturating_duration_since(instant).as_secs_f32() * 20.0)
                .unwrap_or(0.0);
            base + elapsed_ticks
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TitleTimes {
    pub fade_in_ticks: i32,
    pub stay_ticks: i32,
    pub fade_out_ticks: i32,
}

impl Default for TitleTimes {
    fn default() -> Self {
        Self {
            fade_in_ticks: 10,
            stay_ticks: 70,
            fade_out_ticks: 20,
        }
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct TitleOverlayState {
    pub title: String,
    pub subtitle: String,
    pub action_bar: String,
    pub times: TitleTimes,
    pub title_started_at: Option<Instant>,
    pub action_bar_started_at: Option<Instant>,
}

impl TitleOverlayState {
    pub fn clear(&mut self) {
        self.title.clear();
        self.subtitle.clear();
        self.title_started_at = None;
    }

    pub fn reset(&mut self) {
        self.clear();
        self.action_bar.clear();
        self.action_bar_started_at = None;
        self.times = TitleTimes::default();
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct TabListHeaderFooter {
    pub header: String,
    pub footer: String,
}

#[derive(Debug, Clone, Default)]
pub struct ScoreboardObjectiveState {
    pub display_name: String,
    pub render_type: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ScoreboardTeamState {
    pub display_name: String,
    pub prefix: String,
    pub suffix: String,
    pub players: Vec<String>,
}

#[derive(Resource, Debug, Clone, Default)]
pub struct ScoreboardState {
    pub objectives: HashMap<String, ScoreboardObjectiveState>,
    pub display_slots: HashMap<u8, String>,
    pub scores: HashMap<(String, String), i32>,
    pub teams: HashMap<String, ScoreboardTeamState>,
    pub player_teams: HashMap<String, String>,
}

impl ScoreboardState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn set_display_slot(&mut self, position: u8, objective_name: String) {
        if objective_name.is_empty() {
            self.display_slots.remove(&position);
        } else {
            self.display_slots.insert(position, objective_name);
        }
    }

    pub fn remove_objective(&mut self, objective_name: &str) {
        self.objectives.remove(objective_name);
        self.display_slots
            .retain(|_, name| name.as_str() != objective_name);
        self.scores
            .retain(|(_, objective), _| objective.as_str() != objective_name);
    }

    pub fn set_objective(
        &mut self,
        objective_name: String,
        display_name: String,
        render_type: Option<String>,
    ) {
        self.objectives.insert(
            objective_name,
            ScoreboardObjectiveState {
                display_name,
                render_type,
            },
        );
    }

    pub fn set_score(&mut self, entry_name: String, objective_name: String, value: i32) {
        self.scores.insert((entry_name, objective_name), value);
    }

    pub fn remove_score(&mut self, entry_name: &str, objective_name: &str) {
        self.scores
            .remove(&(entry_name.to_string(), objective_name.to_string()));
    }

    pub fn apply_team(
        &mut self,
        team_name: String,
        mode: u8,
        display_name: Option<String>,
        prefix: Option<String>,
        suffix: Option<String>,
        players: Option<Vec<String>>,
    ) {
        match mode {
            0 => {
                let players = players.unwrap_or_default();
                self.detach_players(&players);
                for player in &players {
                    self.player_teams.insert(player.clone(), team_name.clone());
                }
                self.teams.insert(
                    team_name,
                    ScoreboardTeamState {
                        display_name: display_name.unwrap_or_default(),
                        prefix: prefix.unwrap_or_default(),
                        suffix: suffix.unwrap_or_default(),
                        players,
                    },
                );
            }
            1 => {
                if let Some(team) = self.teams.remove(&team_name) {
                    for player in team.players {
                        self.player_teams.remove(&player);
                    }
                }
            }
            2 => {
                let team = self.teams.entry(team_name).or_default();
                if let Some(display_name) = display_name {
                    team.display_name = display_name;
                }
                if let Some(prefix) = prefix {
                    team.prefix = prefix;
                }
                if let Some(suffix) = suffix {
                    team.suffix = suffix;
                }
            }
            3 => {
                let players = players.unwrap_or_default();
                self.detach_players(&players);
                let team = self.teams.entry(team_name.clone()).or_default();
                for player in players {
                    if !team.players.iter().any(|existing| existing == &player) {
                        team.players.push(player.clone());
                    }
                    self.player_teams.insert(player, team_name.clone());
                }
            }
            4 => {
                if let Some(team) = self.teams.get_mut(&team_name) {
                    for player in players.unwrap_or_default() {
                        team.players.retain(|existing| existing != &player);
                        if self
                            .player_teams
                            .get(&player)
                            .is_some_and(|mapped_team| mapped_team == &team_name)
                        {
                            self.player_teams.remove(&player);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub fn sidebar_objective(&self) -> Option<(&str, &ScoreboardObjectiveState)> {
        let name = self.display_slots.get(&1)?;
        let objective = self.objectives.get(name)?;
        Some((name.as_str(), objective))
    }

    pub fn sidebar_lines(&self) -> Vec<(String, i32)> {
        let Some((objective_name, _)) = self.sidebar_objective() else {
            return Vec::new();
        };

        let mut lines: Vec<(String, i32)> = self
            .scores
            .iter()
            .filter(|((entry, objective), _)| {
                objective == objective_name && !entry.starts_with('#')
            })
            .map(|((entry, _), value)| (self.format_entry(entry), *value))
            .collect();
        lines.sort_by(|(name_a, value_a), (name_b, value_b)| {
            value_a.cmp(value_b).then_with(|| name_b.cmp(name_a))
        });
        if lines.len() > 15 {
            lines = lines.split_off(lines.len() - 15);
        }
        lines
    }

    fn format_entry(&self, entry_name: &str) -> String {
        let Some(team_name) = self.player_teams.get(entry_name) else {
            return entry_name.to_string();
        };
        let Some(team) = self.teams.get(team_name) else {
            return entry_name.to_string();
        };
        format!("{}{}{}", team.prefix, entry_name, team.suffix)
    }

    fn detach_players(&mut self, players: &[String]) {
        for player in players {
            if let Some(previous_team) = self.player_teams.remove(player)
                && let Some(team) = self.teams.get_mut(&previous_team)
            {
                team.players.retain(|existing| existing != player);
            }
        }
    }
}

#[derive(Clone)]
pub struct ChunkSection {
    pub y: u8,
    pub blocks: Vec<u16>,
    pub block_light: Vec<u8>,
    pub sky_light: Option<Vec<u8>>,
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
    HeadLook {
        entity_id: i32,
        head_yaw: f32,
    },
    Equipment {
        entity_id: i32,
        slot: u16,
        item: Option<InventoryItemStack>,
    },
    SetItemStack {
        entity_id: i32,
        stack: Option<InventoryItemStack>,
    },
    SheepAppearance {
        entity_id: i32,
        fleece_color: u8,
        sheared: bool,
    },
    Animation {
        entity_id: i32,
        animation: NetEntityAnimation,
    },
    SetLabel {
        entity_id: i32,
        label: String,
    },
    CollectItem {
        collected_entity_id: i32,
        collector_entity_id: i32,
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
    pub gamemode: u8,
    pub can_fly: bool,
    pub flying: bool,
    pub flying_speed: f32,
    pub walking_speed: f32,
    pub speed_effect_amplifier: Option<u8>,
    pub jump_boost_amplifier: Option<u8>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SoundCategory {
    Master,
    Music,
    Record,
    Weather,
    Block,
    Hostile,
    Neutral,
    Player,
    Ambient,
}

impl SoundCategory {
    pub const ALL: [Self; 9] = [
        Self::Master,
        Self::Music,
        Self::Record,
        Self::Weather,
        Self::Block,
        Self::Hostile,
        Self::Neutral,
        Self::Player,
        Self::Ambient,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Master => "Master",
            Self::Music => "Music",
            Self::Record => "Record",
            Self::Weather => "Weather",
            Self::Block => "Block",
            Self::Hostile => "Hostile",
            Self::Neutral => "Neutral",
            Self::Player => "Player",
            Self::Ambient => "Ambient",
        }
    }

    pub const fn from_vanilla_id(id: i32) -> Option<Self> {
        match id {
            0 => Some(Self::Master),
            1 => Some(Self::Music),
            2 => Some(Self::Record),
            3 => Some(Self::Weather),
            4 => Some(Self::Block),
            5 => Some(Self::Hostile),
            6 => Some(Self::Neutral),
            7 => Some(Self::Player),
            8 => Some(Self::Ambient),
            _ => None,
        }
    }
}

#[derive(Resource, Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundSettings {
    pub master: f32,
    pub music: f32,
    pub record: f32,
    pub weather: f32,
    pub block: f32,
    pub hostile: f32,
    pub neutral: f32,
    pub player: f32,
    pub ambient: f32,
}

impl Default for SoundSettings {
    fn default() -> Self {
        Self {
            master: 1.0,
            music: 1.0,
            record: 1.0,
            weather: 1.0,
            block: 1.0,
            hostile: 1.0,
            neutral: 1.0,
            player: 1.0,
            ambient: 1.0,
        }
    }
}

impl SoundSettings {
    pub fn clamp_all(&mut self) {
        self.master = self.master.clamp(0.0, 1.0);
        self.music = self.music.clamp(0.0, 1.0);
        self.record = self.record.clamp(0.0, 1.0);
        self.weather = self.weather.clamp(0.0, 1.0);
        self.block = self.block.clamp(0.0, 1.0);
        self.hostile = self.hostile.clamp(0.0, 1.0);
        self.neutral = self.neutral.clamp(0.0, 1.0);
        self.player = self.player.clamp(0.0, 1.0);
        self.ambient = self.ambient.clamp(0.0, 1.0);
    }

    pub const fn category_gain(self, category: SoundCategory) -> f32 {
        match category {
            SoundCategory::Master => self.master,
            SoundCategory::Music => self.music,
            SoundCategory::Record => self.record,
            SoundCategory::Weather => self.weather,
            SoundCategory::Block => self.block,
            SoundCategory::Hostile => self.hostile,
            SoundCategory::Neutral => self.neutral,
            SoundCategory::Player => self.player,
            SoundCategory::Ambient => self.ambient,
        }
    }

    pub fn final_gain(self, category: SoundCategory, base_gain: f32) -> f32 {
        (self.master * self.category_gain(category) * base_gain).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundStopScope {
    All,
    Category(SoundCategory),
}

#[derive(Debug, Clone)]
pub enum SoundEvent {
    Ui {
        event_id: String,
        volume: f32,
        pitch: f32,
        category_override: Option<SoundCategory>,
    },
    World {
        event_id: String,
        position: Vec3,
        volume: f32,
        pitch: f32,
        category_override: Option<SoundCategory>,
        distance_delay: bool,
    },
    Entity {
        event_id: String,
        entity_id: i32,
        volume: f32,
        pitch: f32,
        category_override: Option<SoundCategory>,
    },
    Stop {
        scope: SoundStopScope,
    },
}

#[derive(Resource, Debug, Default)]
pub struct SoundEventQueue {
    events: Vec<SoundEvent>,
}

impl SoundEventQueue {
    pub fn push(&mut self, event: SoundEvent) {
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<SoundEvent> {
        std::mem::take(&mut self.events)
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
            gamemode: 0,
            can_fly: false,
            flying: false,
            flying_speed: 0.05,
            walking_speed: 0.1,
            speed_effect_amplifier: None,
            jump_boost_amplifier: None,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryEnchantment {
    pub id: i16,
    pub level: i16,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InventoryItemMeta {
    pub display_name: Option<String>,
    pub lore: Vec<String>,
    pub enchantments: Vec<InventoryEnchantment>,
    pub repair_cost: Option<i32>,
    pub unbreakable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryItemStack {
    pub item_id: i32,
    pub count: u8,
    pub damage: i16,
    pub meta: InventoryItemMeta,
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
        self.player_slots.get(idx).cloned().flatten()
    }

    pub fn consume_selected_hotbar_one(&mut self) -> bool {
        let Some(idx) = self.hotbar_slot_index(self.selected_hotbar_slot) else {
            return false;
        };
        let Some(Some(mut stack)) = self.player_slots.get(idx).cloned() else {
            return false;
        };
        if stack.count == 0 {
            return false;
        }
        stack.count = stack.count.saturating_sub(1);
        self.player_slots[idx] = if stack.count == 0 { None } else { Some(stack) };
        true
    }

    pub fn apply_local_click_window(
        &mut self,
        window_id: u8,
        window_unique_slots: usize,
        slot: i16,
        button: u8,
        mode: u8,
    ) -> Option<InventoryItemStack> {
        let mut slots = if window_id == 0 {
            self.player_slots.clone()
        } else {
            self.window_slots
                .get(&window_id)
                .cloned()
                .unwrap_or_default()
        };
        let mut cursor = self.cursor_item.clone();

        match mode {
            0 => apply_mode_normal_click(&mut slots, &mut cursor, slot, button),
            1 => apply_mode_shift_click(&mut slots, &cursor, window_unique_slots, slot),
            2 => apply_mode_number_key(&mut slots, &cursor, slot, button),
            4 => apply_mode_drop(&mut slots, &cursor, slot, button),
            6 => apply_mode_double_click(&mut slots, &mut cursor, slot, button),
            _ => {}
        }

        self.cursor_item = cursor;
        if window_id == 0 {
            self.player_slots = slots;
        } else {
            self.window_slots.insert(window_id, slots);
        }
        self.cursor_item.clone()
    }

    pub fn apply_local_click_player_window(
        &mut self,
        slot: i16,
        button: u8,
        mode: u8,
    ) -> Option<InventoryItemStack> {
        self.apply_local_click_window(0, 0, slot, button, mode)
    }
}

fn apply_outside_click(cursor_item: &mut Option<InventoryItemStack>, button: u8) {
    match cursor_item.clone() {
        Some(_) if button == 0 => {
            *cursor_item = None;
        }
        Some(mut cursor) if button == 1 => {
            cursor.count = cursor.count.saturating_sub(1);
            *cursor_item = if cursor.count == 0 {
                None
            } else {
                Some(cursor)
            };
        }
        _ => {}
    }
}

fn apply_mode_normal_click(
    slots: &mut Vec<Option<InventoryItemStack>>,
    cursor_item: &mut Option<InventoryItemStack>,
    slot: i16,
    button: u8,
) {
    if slot < 0 {
        apply_outside_click(cursor_item, button);
        return;
    }

    let slot_index = slot as usize;
    if slots.len() <= slot_index {
        slots.resize(slot_index + 1, None);
    }

    let mut slot_item = slots[slot_index].clone();
    let mut cursor = cursor_item.clone();

    if button == 0 {
        match (cursor.clone(), slot_item.clone()) {
            (None, Some(_)) => {
                cursor = slot_item;
                slot_item = None;
            }
            (Some(_), None) => {
                slot_item = cursor;
                cursor = None;
            }
            (Some(cur), Some(mut sl)) => {
                if can_stack(&cur, &sl) && sl.count < max_stack_for_item(sl.item_id) {
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
        match (cursor.clone(), slot_item.clone()) {
            (None, Some(mut sl)) => {
                let take = (sl.count.saturating_add(1)) / 2;
                let remain = sl.count.saturating_sub(take);
                cursor = Some(InventoryItemStack {
                    count: take,
                    ..sl.clone()
                });
                slot_item = if remain == 0 {
                    None
                } else {
                    sl.count = remain;
                    Some(sl)
                };
            }
            (Some(mut cur), None) => {
                slot_item = Some(InventoryItemStack {
                    count: 1,
                    ..cur.clone()
                });
                cur.count = cur.count.saturating_sub(1);
                cursor = if cur.count == 0 { None } else { Some(cur) };
            }
            (Some(mut cur), Some(mut sl)) => {
                if can_stack(&cur, &sl) && sl.count < max_stack_for_item(sl.item_id) {
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

    slots[slot_index] = slot_item;
    *cursor_item = cursor;
}

fn apply_mode_shift_click(
    slots: &mut [Option<InventoryItemStack>],
    cursor_item: &Option<InventoryItemStack>,
    window_unique_slots: usize,
    slot: i16,
) {
    if slot < 0 || slots.is_empty() {
        return;
    }
    let slot_index = slot as usize;
    if slot_index >= slots.len() {
        return;
    }
    let Some(mut moving) = slots[slot_index].take() else {
        return;
    };

    let total = slots.len();
    let player_base = total.saturating_sub(36);
    let player_main_start = player_base;
    let player_hotbar_start = player_base + 27;
    let player_end = total;
    let unique = window_unique_slots.min(player_base);

    let mut targets: Vec<std::ops::Range<usize>> = Vec::new();
    if slot_index < unique {
        targets.push(player_main_start..player_hotbar_start.min(player_end));
        targets.push(player_hotbar_start.min(player_end)..player_end);
    } else if (player_main_start..player_hotbar_start.min(player_end)).contains(&slot_index) {
        targets.push(player_hotbar_start.min(player_end)..player_end);
        if unique > 0 {
            targets.push(0..unique);
        }
    } else if (player_hotbar_start.min(player_end)..player_end).contains(&slot_index) {
        targets.push(player_main_start..player_hotbar_start.min(player_end));
        if unique > 0 {
            targets.push(0..unique);
        }
    }

    if targets.is_empty() {
        slots[slot_index] = Some(moving);
        return;
    }

    for range in &targets {
        for idx in range.clone() {
            if moving.count == 0 {
                break;
            }
            let Some(existing) = slots.get_mut(idx) else {
                continue;
            };
            if let Some(stack) = existing.as_mut() {
                if can_stack(stack, &moving) && stack.count < max_stack_for_item(stack.item_id) {
                    let max = max_stack_for_item(stack.item_id);
                    let space = max.saturating_sub(stack.count);
                    let moved = space.min(moving.count);
                    stack.count = stack.count.saturating_add(moved);
                    moving.count = moving.count.saturating_sub(moved);
                }
            }
        }
    }

    for range in targets {
        for idx in range {
            if moving.count == 0 {
                break;
            }
            let Some(existing) = slots.get_mut(idx) else {
                continue;
            };
            if existing.is_none() {
                *existing = Some(moving.clone());
                moving.count = 0;
            }
        }
    }

    if moving.count > 0 {
        slots[slot_index] = Some(moving);
    }
    let _ = cursor_item;
}

fn apply_mode_number_key(
    slots: &mut [Option<InventoryItemStack>],
    cursor_item: &Option<InventoryItemStack>,
    slot: i16,
    button: u8,
) {
    if slot < 0 || button > 8 || slots.len() < 9 {
        return;
    }
    let slot_index = slot as usize;
    let hotbar_start = slots.len() - 9;
    let hotbar_index = hotbar_start + button as usize;
    if slot_index >= slots.len() || hotbar_index >= slots.len() {
        return;
    }
    slots.swap(slot_index, hotbar_index);
    let _ = cursor_item;
}

fn apply_mode_drop(
    slots: &mut [Option<InventoryItemStack>],
    cursor_item: &Option<InventoryItemStack>,
    slot: i16,
    button: u8,
) {
    if slot < 0 {
        return;
    }
    let slot_index = slot as usize;
    if slot_index >= slots.len() {
        return;
    }
    if let Some(mut stack) = slots[slot_index].clone() {
        if button == 0 {
            stack.count = stack.count.saturating_sub(1);
            slots[slot_index] = if stack.count == 0 { None } else { Some(stack) };
        } else {
            slots[slot_index] = None;
        }
    }
    let _ = cursor_item;
}

fn apply_mode_double_click(
    slots: &mut [Option<InventoryItemStack>],
    cursor_item: &mut Option<InventoryItemStack>,
    slot: i16,
    button: u8,
) {
    if button != 0 {
        return;
    }
    let mut cursor = match cursor_item.clone() {
        Some(c) => c,
        None => return,
    };
    let max = max_stack_for_item(cursor.item_id);
    if cursor.count >= max {
        return;
    }

    let skip_slot = if slot >= 0 { Some(slot as usize) } else { None };
    for idx in 0..slots.len() {
        if Some(idx) == skip_slot {
            continue;
        }
        let Some(mut stack) = slots[idx].clone() else {
            continue;
        };
        if !can_stack(&stack, &cursor) {
            continue;
        }
        let need = max.saturating_sub(cursor.count);
        if need == 0 {
            break;
        }
        let moved = need.min(stack.count);
        stack.count = stack.count.saturating_sub(moved);
        cursor.count = cursor.count.saturating_add(moved);
        slots[idx] = if stack.count == 0 { None } else { Some(stack) };
        if cursor.count >= max {
            break;
        }
    }
    *cursor_item = Some(cursor);
}

fn can_stack(a: &InventoryItemStack, b: &InventoryItemStack) -> bool {
    a.item_id == b.item_id && a.damage == b.damage && a.meta == b.meta
}

fn max_stack_for_item(item_id: i32) -> u8 {
    if is_single_stack_item(item_id) { 1 } else { 64 }
}

#[derive(Clone, Copy)]
struct ItemProperties {
    durability: Option<i16>,
    single_stack: bool,
}

fn item_properties(item_id: i32) -> ItemProperties {
    match item_id {
        256 | 269 | 273 | 277 | 284 => ItemProperties {
            durability: Some(59),
            single_stack: true,
        },
        257 | 270 | 274 | 278 | 285 => ItemProperties {
            durability: Some(131),
            single_stack: true,
        },
        258 | 271 | 275 | 279 | 286 => ItemProperties {
            durability: Some(250),
            single_stack: true,
        },
        259 => ItemProperties {
            durability: Some(64),
            single_stack: true,
        },
        261 => ItemProperties {
            durability: Some(384),
            single_stack: true,
        },
        267 | 272 | 276 | 283 => ItemProperties {
            durability: Some(32),
            single_stack: true,
        },
        268 => ItemProperties {
            durability: Some(59),
            single_stack: true,
        },
        290 | 291 | 292 | 294 => ItemProperties {
            durability: Some(59),
            single_stack: true,
        },
        293 => ItemProperties {
            durability: Some(131),
            single_stack: true,
        },
        298 => ItemProperties {
            durability: Some(55),
            single_stack: true,
        },
        299 => ItemProperties {
            durability: Some(80),
            single_stack: true,
        },
        300 => ItemProperties {
            durability: Some(75),
            single_stack: true,
        },
        301 => ItemProperties {
            durability: Some(65),
            single_stack: true,
        },
        302 => ItemProperties {
            durability: Some(165),
            single_stack: true,
        },
        303 => ItemProperties {
            durability: Some(240),
            single_stack: true,
        },
        304 => ItemProperties {
            durability: Some(225),
            single_stack: true,
        },
        305 => ItemProperties {
            durability: Some(195),
            single_stack: true,
        },
        306 | 310 => ItemProperties {
            durability: Some(363),
            single_stack: true,
        },
        307 | 311 => ItemProperties {
            durability: Some(528),
            single_stack: true,
        },
        308 | 312 => ItemProperties {
            durability: Some(495),
            single_stack: true,
        },
        309 | 313 => ItemProperties {
            durability: Some(429),
            single_stack: true,
        },
        314 => ItemProperties {
            durability: Some(77),
            single_stack: true,
        },
        315 => ItemProperties {
            durability: Some(112),
            single_stack: true,
        },
        316 => ItemProperties {
            durability: Some(105),
            single_stack: true,
        },
        317 => ItemProperties {
            durability: Some(91),
            single_stack: true,
        },
        346 => ItemProperties {
            durability: Some(64),
            single_stack: true,
        },
        359 => ItemProperties {
            durability: Some(238),
            single_stack: true,
        },
        326..=330 => ItemProperties {
            durability: None,
            single_stack: true,
        },
        _ => ItemProperties {
            durability: None,
            single_stack: false,
        },
    }
}

pub fn item_max_durability(item_id: i32) -> Option<i16> {
    item_properties(item_id).durability
}

fn is_single_stack_item(item_id: i32) -> bool {
    item_properties(item_id).single_stack
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

pub enum ScoreboardMessage {
    Display {
        position: u8,
        objective_name: String,
    },
    Objective {
        name: String,
        mode: Option<u8>,
        display_name: String,
        render_type: Option<String>,
    },
    UpdateScore {
        entry_name: String,
        action: u8,
        objective_name: String,
        value: Option<i32>,
    },
    Team {
        name: String,
        mode: u8,
        display_name: Option<String>,
        prefix: Option<String>,
        suffix: Option<String>,
        players: Option<Vec<String>>,
    },
}

pub enum TitleMessage {
    SetTitle {
        text: String,
    },
    SetSubtitle {
        text: String,
    },
    SetActionBar {
        text: String,
    },
    SetTimes {
        fade_in_ticks: i32,
        stay_ticks: i32,
        fade_out_ticks: i32,
    },
    Clear,
    Reset,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoreboard_sidebar_applies_team_prefix_and_suffix() {
        let mut scoreboard = ScoreboardState::default();
        scoreboard.set_objective(
            "bedwars".to_string(),
            "Bedwars".to_string(),
            Some("integer".to_string()),
        );
        scoreboard.set_display_slot(1, "bedwars".to_string());
        scoreboard.apply_team(
            "red".to_string(),
            0,
            Some("Red".to_string()),
            Some("§c[R] ".to_string()),
            Some(" §7*".to_string()),
            Some(vec!["Alice".to_string()]),
        );
        scoreboard.set_score("Alice".to_string(), "bedwars".to_string(), 12);

        let lines = scoreboard.sidebar_lines();
        assert_eq!(lines, vec![("§c[R] Alice §7*".to_string(), 12)]);
    }

    #[test]
    fn scoreboard_remove_objective_clears_sidebar_slot_and_scores() {
        let mut scoreboard = ScoreboardState::default();
        scoreboard.set_objective("bw".to_string(), "Bedwars".to_string(), None);
        scoreboard.set_display_slot(1, "bw".to_string());
        scoreboard.set_score("Alice".to_string(), "bw".to_string(), 5);

        scoreboard.remove_objective("bw");

        assert!(scoreboard.sidebar_objective().is_none());
        assert!(scoreboard.sidebar_lines().is_empty());
    }

    #[test]
    fn scoreboard_sidebar_filters_hidden_entries_and_keeps_highest_fifteen() {
        let mut scoreboard = ScoreboardState::default();
        scoreboard.set_objective("bw".to_string(), "Bedwars".to_string(), None);
        scoreboard.set_display_slot(1, "bw".to_string());

        scoreboard.set_score("#hidden".to_string(), "bw".to_string(), 999);
        for idx in 0..20 {
            scoreboard.set_score(format!("Player{idx:02}"), "bw".to_string(), idx);
        }

        let lines = scoreboard.sidebar_lines();
        assert_eq!(lines.len(), 15);
        assert_eq!(lines.first(), Some(&("Player05".to_string(), 5)));
        assert_eq!(lines.last(), Some(&("Player19".to_string(), 19)));
        assert!(lines.iter().all(|(name, _)| !name.starts_with('#')));
    }

    #[test]
    fn sound_settings_final_gain_respects_master_and_category() {
        let settings = SoundSettings {
            master: 0.5,
            block: 0.4,
            ..Default::default()
        };
        assert!((settings.final_gain(SoundCategory::Block, 0.75) - 0.15).abs() < 1e-6);
    }
}
