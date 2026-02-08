use bevy::core_pipeline::Skybox;
use bevy::prelude::*;

use crate::components::{LookAngles, Player, PlayerCamera, Velocity};

pub fn spawn_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let skybox_handle = asset_server.load("skybox.ktx2");

    commands
        .spawn((
            Player,
            Velocity::default(),
            LookAngles::default(),
            Transform::from_xyz(0.0, 2.0, 8.0),
            GlobalTransform::default(),
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
                Transform::from_xyz(0.0, 0.0, 0.0),
            ));
        });
}
