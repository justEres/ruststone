use bevy::{ecs::resource::Resource, prelude::Vec3};
use serde::{Deserialize, Serialize};

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
