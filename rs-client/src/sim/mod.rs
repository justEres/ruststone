use bevy::prelude::{Resource, Vec3};

pub mod collision;
pub mod movement;
pub mod predict;
pub mod reconcile;
pub mod types;

pub use types::{InputState, PlayerSimState, PredictedFrame};

#[derive(Debug, Resource, Clone, Copy, PartialEq, Eq)]
pub enum CameraPerspectiveMode {
    FirstPerson,
    ThirdPersonBack,
    ThirdPersonFront,
}

impl Default for CameraPerspectiveMode {
    fn default() -> Self {
        Self::FirstPerson
    }
}

#[derive(Debug, Resource, Clone, Copy)]
pub struct CameraPerspectiveState {
    pub mode: CameraPerspectiveMode,
    pub third_person_distance: f32,
}

impl Default for CameraPerspectiveState {
    fn default() -> Self {
        Self {
            mode: CameraPerspectiveMode::FirstPerson,
            third_person_distance: 4.0,
        }
    }
}

#[derive(Debug, Default, Resource)]
pub struct CameraPerspectiveAltHold {
    pub saved_mode: Option<CameraPerspectiveMode>,
}

#[derive(Debug, Resource, Clone, Copy)]
pub struct LocalArmSwing {
    /// 0.0 = just started, 1.0 = finished
    pub progress: f32,
}

impl Default for LocalArmSwing {
    fn default() -> Self {
        Self { progress: 1.0 }
    }
}

#[derive(Debug, Default, Resource)]
pub struct SimClock {
    pub tick: u32,
}

#[derive(Debug, Default, Resource)]
pub struct CurrentInput(pub InputState);

#[derive(Debug, Default, Resource)]
pub struct SimState {
    pub current: PlayerSimState,
}

#[derive(Debug, Default, Resource)]
pub struct SimRenderState {
    pub previous: PlayerSimState,
}

#[derive(Debug, Default, Resource)]
pub struct VisualCorrectionOffset(pub Vec3);

#[derive(Debug, Default, Resource)]
pub struct DebugStats {
    pub last_correction: f32,
    pub last_replay: u32,
    pub smoothing_offset_len: f32,
    pub one_way_ticks: u32,
}

#[derive(Debug, Default, Resource)]
pub struct SimReady(pub bool);

#[derive(Debug, Resource, Clone, Copy)]
pub struct CorrectionLoopGuard {
    pub last_server_pos: Vec3,
    pub last_server_on_ground: bool,
    pub repeats: u32,
    pub skip_send_ticks: u32,
    pub force_full_pos_ticks: u32,
    pub skip_physics_ticks: u32,
    pub pending_ack: Option<(Vec3, f32, f32, bool)>,
}

impl Default for CorrectionLoopGuard {
    fn default() -> Self {
        Self {
            last_server_pos: Vec3::ZERO,
            last_server_on_ground: false,
            repeats: 0,
            skip_send_ticks: 0,
            force_full_pos_ticks: 0,
            skip_physics_ticks: 0,
            pending_ack: None,
        }
    }
}

#[derive(Debug, Resource, Clone, Copy)]
pub struct MovementPacketState {
    pub initialized: bool,
    pub last_pos: Vec3,
    pub last_yaw_deg: f32,
    pub last_pitch_deg: f32,
    pub ticks_since_pos: u32,
}

impl Default for MovementPacketState {
    fn default() -> Self {
        Self {
            initialized: false,
            last_pos: Vec3::ZERO,
            last_yaw_deg: 0.0,
            last_pitch_deg: 0.0,
            ticks_since_pos: 0,
        }
    }
}

#[derive(Debug, Resource, Clone, Copy)]
pub struct ZoomState {
    pub active: bool,
    pub current_factor: f32,
    pub target_factor: f32,
    pub wheel_factor: f32,
}

impl Default for ZoomState {
    fn default() -> Self {
        Self {
            active: false,
            current_factor: 1.0,
            target_factor: 1.0,
            wheel_factor: 1.0,
        }
    }
}

#[derive(Debug, Resource)]
pub struct DebugUiState {
    pub open: bool,
    pub show_prediction: bool,
    pub show_performance: bool,
    pub show_render: bool,
    pub render_show_layers: bool,
    pub perf_show_schedules: bool,
    pub perf_show_render_stats: bool,
}

impl Default for DebugUiState {
    fn default() -> Self {
        Self {
            open: false,
            show_prediction: false,
            show_performance: false,
            show_render: false,
            render_show_layers: false,
            perf_show_schedules: false,
            perf_show_render_stats: false,
        }
    }
}

#[cfg(test)]
mod tests;
