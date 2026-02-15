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
}

impl Default for DebugUiState {
    fn default() -> Self {
        Self {
            open: false,
            show_prediction: true,
            show_performance: true,
            show_render: true,
        }
    }
}

#[cfg(test)]
mod tests;
