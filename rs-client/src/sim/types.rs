use bevy::prelude::Vec3;

#[derive(Clone, Copy, Debug, Default)]
pub struct InputState {
    pub forward: f32,
    pub strafe: f32,
    pub jump: bool,
    pub sprint: bool,
    pub sneak: bool,
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct PlayerSimState {
    pub pos: Vec3,
    pub vel: Vec3,
    pub on_ground: bool,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for PlayerSimState {
    fn default() -> Self {
        Self {
            pos: Vec3::ZERO,
            vel: Vec3::ZERO,
            on_ground: false,
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PredictedFrame {
    pub tick: u32,
    pub input: InputState,
    pub state: PlayerSimState,
}
