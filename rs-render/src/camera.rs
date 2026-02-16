use bevy::core_pipeline::Skybox;
use bevy::core_pipeline::fxaa::{Fxaa, Sensitivity};
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::render::view::Msaa;

use crate::components::{LookAngles, Player, PlayerCamera, Velocity};
use crate::debug::{AntiAliasingMode, RenderDebugSettings};
use crate::reflection::{MAIN_RENDER_LAYER, REFLECTION_RENDER_LAYER};

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
                Fxaa {
                    enabled: matches!(
                        debug_settings.aa_mode,
                        AntiAliasingMode::Fxaa | AntiAliasingMode::Msaa4 | AntiAliasingMode::Msaa8
                    ),
                    edge_threshold: Sensitivity::Ultra,
                    edge_threshold_min: Sensitivity::High,
                },
                Msaa::Off,
                Skybox {
                    image: skybox_handle,
                    brightness: 1000.0,
                    ..Default::default()
                },
                Projection::Perspective(PerspectiveProjection {
                    fov: debug_settings.fov_deg.to_radians(),
                    ..Default::default()
                }),
                RenderLayers::layer(MAIN_RENDER_LAYER).with(REFLECTION_RENDER_LAYER),
                Transform::from_xyz(0.0, EYE_HEIGHT, 0.0),
                Visibility::Inherited,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ));
        });
}
