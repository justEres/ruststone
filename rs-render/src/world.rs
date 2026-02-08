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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    settings: Res<WorldSettings>,
) {
    let root = commands
        .spawn((
            WorldRoot,
            Transform::default(),
            GlobalTransform::default(),
        ))
        .id();

    commands.entity(root).with_children(|parent| {
        let ground_mesh = Mesh::from(Plane3d::default());
        parent.spawn((
            Mesh3d(meshes.add(ground_mesh)),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: settings.ground_color,
                perceptual_roughness: 1.0,
                ..default()
            })),
            Transform::from_scale(Vec3::splat(settings.ground_size)),
        ));

        let marker_mesh = Mesh::from(Cuboid::new(1.0, 1.0, 1.0));
        parent.spawn((
            Mesh3d(meshes.add(marker_mesh)),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.2, 0.6, 0.9),
                ..default()
            })),
            Transform::from_xyz(3.0, 0.5, -3.0),
        ));
    });

    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            illuminance: 25_000.0,
            ..default()
        },
        Transform::from_xyz(8.0, 16.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.45, 0.45, 0.5),
        brightness: 0.35,
        affects_lightmapped_meshes: true,
    });
}
