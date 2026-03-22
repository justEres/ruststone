use bevy::pbr::{CascadeShadowConfigBuilder, NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bevy::render::view::NoFrustumCulling;
use bevy::render::view::RenderLayers;

use crate::components::{ShadowCasterLight, WorldRoot};
use crate::debug::RenderDebugSettings;
use crate::lighting::effective_sun_direction;
use crate::reflection::{
    CHUNK_CUTOUT_RENDER_LAYER, CHUNK_OPAQUE_RENDER_LAYER, CHUNK_TRANSPARENT_RENDER_LAYER,
    LOCAL_PLAYER_RENDER_LAYER, MAIN_RENDER_LAYER,
};
use rs_utils::WorldTime;

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

#[derive(Component)]
pub struct SunSprite;

pub fn setup_world(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    _settings: Res<WorldSettings>,
) {
    let _root = commands.spawn((WorldRoot, Transform::default(), GlobalTransform::default()));

    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            illuminance: 12_000.0,
            shadow_depth_bias: 0.02,
            shadow_normal_bias: 0.6,
            ..default()
        },
        CascadeShadowConfigBuilder::default().build(),
        ShadowCasterLight,
        Transform::from_xyz(8.0, 16.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(MAIN_RENDER_LAYER)
            .with(CHUNK_OPAQUE_RENDER_LAYER)
            .with(CHUNK_CUTOUT_RENDER_LAYER)
            .with(CHUNK_TRANSPARENT_RENDER_LAYER)
            .with(LOCAL_PLAYER_RENDER_LAYER),
    ));

    commands.insert_resource(AmbientLight {
        color: Color::srgb(0.6, 0.6, 0.65),
        brightness: 0.9,
        affects_lightmapped_meshes: true,
    });

    let sun_texture =
        asset_server.load("texturepack/assets/minecraft/textures/environment/sun.png");
    let sun_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(sun_texture),
        emissive: LinearRgba::rgb(2.5, 2.35, 2.1),
        alpha_mode: AlphaMode::Add,
        cull_mode: None,
        unlit: true,
        depth_bias: -100.0,
        ..default()
    });
    commands.spawn((
        SunSprite,
        Mesh3d(meshes.add(Rectangle::new(60.0, 60.0))),
        MeshMaterial3d(sun_material),
        Transform::from_translation(Vec3::new(0.0, 140.0, 0.0)),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        ViewVisibility::default(),
        NoFrustumCulling,
        NotShadowCaster,
        NotShadowReceiver,
        RenderLayers::layer(MAIN_RENDER_LAYER)
            .with(CHUNK_OPAQUE_RENDER_LAYER)
            .with(CHUNK_CUTOUT_RENDER_LAYER)
            .with(CHUNK_TRANSPARENT_RENDER_LAYER)
            .with(LOCAL_PLAYER_RENDER_LAYER),
    ));
}

pub fn update_sun_sprite(
    settings: Res<RenderDebugSettings>,
    world_time: Res<WorldTime>,
    camera_query: Query<&GlobalTransform, With<crate::components::PlayerCamera>>,
    mut sun_query: Query<(&mut Transform, &mut Visibility), With<SunSprite>>,
) {
    let Ok(camera) = camera_query.get_single() else {
        return;
    };
    let Ok((mut transform, mut visibility)) = sun_query.get_single_mut() else {
        return;
    };

    *visibility = if settings.render_sun_sprite {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if !settings.render_sun_sprite {
        return;
    }

    let sun_dir = effective_sun_direction(&settings, Some(&world_time));
    let sun_distance = 460.0;
    let sun_pos = camera.translation() + sun_dir * sun_distance;
    transform.translation = sun_pos;
    transform.look_at(camera.translation(), Vec3::Y);
}
