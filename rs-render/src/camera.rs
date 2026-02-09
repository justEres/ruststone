use bevy::core_pipeline::Skybox;
use bevy::prelude::*;

use crate::components::{LookAngles, Player, PlayerCamera, Velocity};
use crate::debug::RenderDebugSettings;

const EYE_HEIGHT: f32 = 1.62;

pub fn spawn_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    debug_settings: Res<RenderDebugSettings>,
) {
    let skybox_handle = asset_server.load("skybox.ktx2");

    commands
        .spawn((
            Player,
            Velocity::default(),
            LookAngles::default(),
            Transform::from_xyz(0.0, 0.0, 0.0),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Camera3d::default(),
                PlayerCamera,
                Skybox {
                    image: skybox_handle,
                    brightness: 1000.0,
                    ..Default::default()
                },
                Projection::Perspective(PerspectiveProjection {
                    fov: debug_settings.fov_deg.to_radians(),
                    ..Default::default()
                }),
                Transform::from_xyz(0.0, EYE_HEIGHT, 0.0),
                Visibility::Inherited,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ));
        });
}
