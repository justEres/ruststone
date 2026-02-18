use bevy::core_pipeline::Skybox;
use bevy::prelude::*;
use bevy::render::camera::CameraProjection;
use bevy::render::view::RenderLayers;
use bevy::window::PrimaryWindow;

use crate::chunk::{ChunkRenderAssets, create_reflection_target_image};
use crate::components::PlayerCamera;
use crate::debug::RenderDebugSettings;

pub const MAIN_RENDER_LAYER: usize = 0;
pub const REFLECTION_RENDER_LAYER: usize = 1;
pub const DEFAULT_WATER_PLANE_Y: f32 = 62.0;

#[derive(Component)]
pub struct ReflectionCamera;

#[derive(Resource)]
pub struct ReflectionPassState {
    pub texture: Handle<Image>,
    pub camera: Option<Entity>,
    pub plane_y: f32,
    pub view_proj: Mat4,
    pub width: u32,
    pub height: u32,
}

impl FromWorld for ReflectionPassState {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<ChunkRenderAssets>();
        Self {
            texture: assets.reflection_texture.clone(),
            camera: None,
            plane_y: DEFAULT_WATER_PLANE_Y,
            view_proj: Mat4::IDENTITY,
            width: 1024,
            height: 1024,
        }
    }
}

pub fn spawn_reflection_camera(
    mut commands: Commands,
    mut state: ResMut<ReflectionPassState>,
    main_camera_query: Query<&Projection, With<PlayerCamera>>,
    main_skybox_query: Query<&Skybox, With<PlayerCamera>>,
) {
    if state.camera.is_some() {
        return;
    }
    let Ok(main_projection) = main_camera_query.single() else {
        return;
    };

    let mut entity_commands = commands.spawn((
        Name::new("WaterReflectionCamera"),
        ReflectionCamera,
        Camera3d::default(),
        Camera {
            target: state.texture.clone().into(),
            order: -100,
            is_active: false,
            // Keep non-rendered regions closer to sky color to avoid black SSR artifacts.
            clear_color: Color::srgb(0.50, 0.62, 0.84).into(),
            ..default()
        },
        main_projection.clone(),
        Transform::IDENTITY,
        RenderLayers::layer(REFLECTION_RENDER_LAYER),
    ));
    if let Ok(skybox) = main_skybox_query.single() {
        entity_commands.insert(skybox.clone());
    }
    let entity = entity_commands.id();
    state.camera = Some(entity);
}

pub fn sync_reflection_camera(
    settings: Res<RenderDebugSettings>,
    main_camera_query: Query<
        (&GlobalTransform, &Projection),
        (With<PlayerCamera>, Without<ReflectionCamera>),
    >,
    mut reflection_query: Query<
        (&mut Transform, &mut Projection, &mut Camera),
        (With<ReflectionCamera>, Without<PlayerCamera>),
    >,
    mut state: ResMut<ReflectionPassState>,
) {
    let Ok((main_transform, main_projection)) = main_camera_query.single() else {
        return;
    };
    let Ok((mut reflection_transform, mut reflection_projection, mut reflection_camera)) =
        reflection_query.single_mut()
    else {
        return;
    };

    let active = settings.water_reflections_enabled && settings.water_terrain_ssr;
    reflection_camera.is_active = active;
    *reflection_projection = main_projection.clone();
    if let Projection::Perspective(p) = &mut *reflection_projection {
        p.fov = (p.fov * settings.water_reflection_overscan.clamp(1.0, 3.0))
            .clamp(15.0f32.to_radians(), 170.0f32.to_radians());
    }

    let cam_pos = main_transform.translation();
    let reflected_pos = Vec3::new(cam_pos.x, 2.0 * state.plane_y - cam_pos.y, cam_pos.z);
    let fwd = main_transform.forward();
    let up = main_transform.up();
    let reflected_fwd = Vec3::new(fwd.x, -fwd.y, fwd.z).normalize_or_zero();
    let reflected_up = Vec3::new(up.x, -up.y, up.z).normalize_or_zero();

    let mut transform = Transform::from_translation(reflected_pos);
    transform.look_to(reflected_fwd, reflected_up);
    *reflection_transform = transform;

    let clip_from_view = reflection_projection.get_clip_from_view();
    let view_from_world = reflection_transform.compute_matrix().inverse();
    state.view_proj = clip_from_view * view_from_world;
}

pub fn resize_reflection_target(
    settings: Res<RenderDebugSettings>,
    mut images: ResMut<Assets<Image>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut state: ResMut<ReflectionPassState>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    // Keep reflection target slightly oversized/undersized via user scale to reduce borders.
    let scale = settings.water_reflection_resolution_scale.clamp(0.5, 3.0);
    let desired_width = ((window.physical_width().max(2) as f32) * scale) as u32;
    let desired_height = ((window.physical_height().max(2) as f32) * scale) as u32;
    let desired_width = desired_width.max(512);
    let desired_height = desired_height.max(512);
    if desired_width == state.width && desired_height == state.height {
        return;
    }
    if let Some(image) = images.get_mut(&state.texture) {
        *image = create_reflection_target_image(desired_width, desired_height);
        state.width = desired_width;
        state.height = desired_height;
    }
}
