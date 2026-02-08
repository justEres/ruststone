use bevy::prelude::*;

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct PlayerCamera;

#[derive(Component, Default)]
pub struct Velocity(pub Vec3);

#[derive(Component)]
pub struct LookAngles {
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for LookAngles {
    fn default() -> Self {
        Self { yaw: 0.0, pitch: 0.0 }
    }
}

#[derive(Component)]
pub struct WorldRoot;
