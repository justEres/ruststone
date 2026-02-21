use bevy::core_pipeline::Skybox;
use bevy::core_pipeline::core_3d::Camera3dDepthLoadOp;
use bevy::core_pipeline::fxaa::{Fxaa, Sensitivity};
use bevy::prelude::*;
use bevy::render::camera::ClearColorConfig;
use bevy::render::view::RenderLayers;
use bevy::render::view::Msaa;

use crate::components::{LookAngles, Player, PlayerCamera, Velocity, WaterPassCamera};
use crate::debug::{AntiAliasingMode, RenderDebugSettings};
use crate::reflection::{
    CHUNK_CUTOUT_RENDER_LAYER, CHUNK_OPAQUE_RENDER_LAYER, CHUNK_TRANSPARENT_RENDER_LAYER,
    MAIN_RENDER_LAYER,
};

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
                RenderLayers::layer(MAIN_RENDER_LAYER)
                    .with(CHUNK_OPAQUE_RENDER_LAYER)
                    .with(CHUNK_CUTOUT_RENDER_LAYER)
                    .with(CHUNK_TRANSPARENT_RENDER_LAYER),
                Transform::from_xyz(0.0, EYE_HEIGHT, 0.0),
                Visibility::Inherited,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ));
            parent.spawn((
                Camera3d {
                    depth_load_op: Camera3dDepthLoadOp::Load,
                    ..default()
                },
                Camera {
                    order: 1,
                    is_active: false,
                    clear_color: ClearColorConfig::None,
                    ..default()
                },
                WaterPassCamera,
                Msaa::Off,
                Projection::Perspective(PerspectiveProjection {
                    fov: debug_settings.fov_deg.to_radians(),
                    ..Default::default()
                }),
                RenderLayers::layer(CHUNK_TRANSPARENT_RENDER_LAYER),
                Transform::from_xyz(0.0, EYE_HEIGHT, 0.0),
                Visibility::Inherited,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ));
        });
}

pub fn sync_water_pass_camera(
    settings: Res<RenderDebugSettings>,
    main_query: Query<(&Projection, &Camera, &Transform), (With<PlayerCamera>, Without<WaterPassCamera>)>,
    mut water_query: Query<
        (&mut Projection, &mut Camera, &mut Transform, &mut RenderLayers),
        With<WaterPassCamera>,
    >,
) {
    let Ok((main_proj, main_camera, main_tf)) = main_query.single() else {
        return;
    };
    for (mut water_proj, mut water_camera, mut water_tf, mut water_layers) in &mut water_query {
        *water_proj = main_proj.clone();
        water_camera.viewport = main_camera.viewport.clone();
        // Disabled until dedicated render-graph compositing is in place.
        water_camera.is_active = false;
        *water_tf = *main_tf;
        *water_layers = if settings.show_layer_chunks_transparent {
            RenderLayers::layer(CHUNK_TRANSPARENT_RENDER_LAYER)
        } else {
            RenderLayers::none()
        };
    }
}
