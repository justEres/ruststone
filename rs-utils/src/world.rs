use std::time::Instant;

use bevy::ecs::resource::Resource;

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

#[derive(Debug, Clone, Copy)]
pub struct ChestAction {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block_id: u16,
    pub open_count: u8,
}

#[derive(Clone)]
pub struct PlayerPosition {
    pub position: Option<(f64, f64, f64)>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
    pub flags: Option<u8>,
    pub on_ground: Option<bool>,
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

#[derive(Resource, Debug, Clone, Default)]
pub struct TabListHeaderFooter {
    pub header: String,
    pub footer: String,
}
