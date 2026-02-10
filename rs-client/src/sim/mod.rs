use bevy::prelude::{Resource, Vec3};

pub mod collision;
pub mod movement;
pub mod predict;
pub mod reconcile;
pub mod types;

pub use types::{InputState, PlayerSimState, PredictedFrame};

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
