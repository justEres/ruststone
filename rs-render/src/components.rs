use bevy::prelude::*;

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct PlayerCamera;

#[derive(Component)]
pub struct WaterPassCamera;

#[derive(Component, Default)]
pub struct Velocity(pub Vec3);

#[derive(Component)]
pub struct LookAngles {
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for LookAngles {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

#[derive(Component)]
pub struct WorldRoot;

#[derive(Component)]
pub struct ShadowCasterLight;

#[derive(Component, Clone, Copy)]
pub struct ChunkRoot {
    pub key: (i32, i32),
}
