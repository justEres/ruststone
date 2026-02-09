use bevy::prelude::*;

use crate::components::WorldRoot;

#[derive(Resource)]
pub struct WorldSettings {
    pub ground_size: f32,
    pub ground_color: Color,
}

impl Default for WorldSettings {
    fn default() -> Self {
        Self {
            ground_size: 64.0,
            ground_color: Color::srgb(0.2, 0.2, 0.22),
        }
    }
}

pub fn setup_world(
    mut commands: Commands,
    _settings: Res<WorldSettings>,
) {
    let _root = commands.spawn((WorldRoot, Transform::default(), GlobalTransform::default()));

    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            illuminance: 14_000.0,
            ..default()
        },
        Transform::from_xyz(8.0, 16.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.55, 0.55, 0.6),
        brightness: 0.75,
        affects_lightmapped_meshes: true,
    });
}
