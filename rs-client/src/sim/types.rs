use bevy::prelude::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct InputState {
    pub forward: f32,
    pub strafe: f32,
    pub jump: bool,
    pub sprint: bool,
    pub sneak: bool,
    pub can_fly: bool,
    pub flying: bool,
    pub flying_speed: f32,
    pub speed_multiplier: f32,
    pub jump_boost_amplifier: Option<u8>,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            forward: 0.0,
            strafe: 0.0,
            jump: false,
            sprint: false,
            sneak: false,
            can_fly: false,
            flying: false,
            flying_speed: 0.05,
            speed_multiplier: 1.0,
            jump_boost_amplifier: None,
            yaw: 0.0,
            pitch: 0.0,
        }
    }
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
