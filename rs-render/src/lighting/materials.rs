use bevy::pbr::{
    CascadeShadowConfig, CascadeShadowConfigBuilder, DirectionalLightShadowMap,
    OpaqueRendererMethod,
};
use bevy::prelude::*;

use crate::chunk::{ChunkAtlasMaterial, ChunkRenderAssets};
use crate::components::ShadowCasterLight;
use crate::debug::{RenderDebugSettings, RenderPerfStats};
use rs_utils::WorldTime;

use super::presets::uses_shadowed_pbr_path;
use super::uniforms::{
    cutout_alpha_mode, effective_sun_direction, lighting_uniform_for_mode, sun_color_from_warmth,
    water_reflection_mode,
};

pub fn apply_lighting_quality(
    settings: Res<RenderDebugSettings>,
    world_time: Res<WorldTime>,
    mut assets: ResMut<ChunkRenderAssets>,
    mut materials: ResMut<Assets<ChunkAtlasMaterial>>,
    mut chunk_materials: Query<&mut MeshMaterial3d<ChunkAtlasMaterial>>,
    mut lights: Query<(
        &mut DirectionalLight,
        &mut Transform,
        Option<&ShadowCasterLight>,
        Option<&mut CascadeShadowConfig>,
    )>,
    mut shadow_map: ResMut<DirectionalLightShadowMap>,
    mut ambient: ResMut<AmbientLight>,
    mut last_material_key: Local<Option<(u8, bool, u32)>>,
) {
    let cutout_alpha_mode = cutout_alpha_mode(&settings);
    let grass_overlay_info = assets.grass_overlay_info;
    let make_lighting = |pass_mode: f32| {
        let mut u = lighting_uniform_for_mode(&settings, Some(&world_time), pass_mode);
        u.grass_overlay_info = grass_overlay_info;
        u
    };
    let material_key = (
        settings.shader_quality_mode,
        settings.enable_pbr_terrain_lighting,
        settings.material_rebuild_nonce,
    );
    let recreate_materials = last_material_key.map(|k| k != material_key).unwrap_or(true);
    *last_material_key = Some(material_key);

    if recreate_materials {
        let use_shadowed_pbr = uses_shadowed_pbr_path(&settings);
        let old_opaque = assets.opaque_material.clone();
        let old_cutout = assets.cutout_material.clone();
        let old_cutout_culled = assets.cutout_culled_material.clone();
        let old_transparent = assets.transparent_material.clone();

        let opaque_material = materials.add(ChunkAtlasMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: None,
                metallic: 0.0,
                reflectance: 0.0,
                perceptual_roughness: 1.0,
                opaque_render_method: OpaqueRendererMethod::Forward,
                unlit: !use_shadowed_pbr,
                ..default()
            },
            extension: crate::chunk::AtlasTextureExtension {
                atlas: assets.atlas.clone(),
                skybox: assets.skybox_texture.clone(),
                lighting: make_lighting(0.0),
            },
        });
        let transparent_material = materials.add(ChunkAtlasMaterial {
            base: StandardMaterial {
                base_color: Color::srgba(1.0, 1.0, 1.0, 0.8),
                base_color_texture: None,
                metallic: 0.0,
                reflectance: if settings.water_reflections_enabled {
                    0.9
                } else {
                    0.0
                },
                perceptual_roughness: if settings.water_reflections_enabled {
                    0.08
                } else {
                    1.0
                },
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                opaque_render_method: OpaqueRendererMethod::Forward,
                unlit: !use_shadowed_pbr,
                ..default()
            },
            extension: crate::chunk::AtlasTextureExtension {
                atlas: assets.atlas.clone(),
                skybox: assets.skybox_texture.clone(),
                lighting: make_lighting(1.0),
            },
        });
        let cutout_material = materials.add(ChunkAtlasMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: None,
                metallic: 0.0,
                reflectance: 0.0,
                perceptual_roughness: 1.0,
                alpha_mode: cutout_alpha_mode,
                cull_mode: None,
                opaque_render_method: OpaqueRendererMethod::Forward,
                unlit: false,
                ..default()
            },
            extension: crate::chunk::AtlasTextureExtension {
                atlas: assets.atlas.clone(),
                skybox: assets.skybox_texture.clone(),
                lighting: make_lighting(2.0),
            },
        });
        let cutout_culled_material = materials.add(ChunkAtlasMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: None,
                metallic: 0.0,
                reflectance: 0.0,
                perceptual_roughness: 1.0,
                alpha_mode: cutout_alpha_mode,
                cull_mode: Some(bevy::render::render_resource::Face::Back),
                opaque_render_method: OpaqueRendererMethod::Forward,
                unlit: false,
                ..default()
            },
            extension: crate::chunk::AtlasTextureExtension {
                atlas: assets.atlas.clone(),
                skybox: assets.skybox_texture.clone(),
                lighting: make_lighting(2.0),
            },
        });

        assets.opaque_material = opaque_material;
        assets.cutout_material = cutout_material;
        assets.cutout_culled_material = cutout_culled_material;
        assets.transparent_material = transparent_material;

        for mut mat in &mut chunk_materials {
            if mat.0 == old_opaque {
                mat.0 = assets.opaque_material.clone();
            } else if mat.0 == old_cutout {
                mat.0 = assets.cutout_material.clone();
            } else if mat.0 == old_cutout_culled {
                mat.0 = assets.cutout_culled_material.clone();
            } else if mat.0 == old_transparent {
                mat.0 = assets.transparent_material.clone();
            }
        }
    }

    if let Some(mat) = materials.get_mut(&assets.opaque_material) {
        mat.extension.lighting = make_lighting(0.0);
        mat.base.unlit = !uses_shadowed_pbr_path(&settings);
        mat.base.alpha_mode = AlphaMode::Opaque;
        mat.base.opaque_render_method = OpaqueRendererMethod::Forward;
        mat.base.perceptual_roughness = 1.0;
        mat.base.reflectance = 0.02;
    }
    if let Some(mat) = materials.get_mut(&assets.cutout_material) {
        mat.extension.lighting = make_lighting(2.0);
        mat.base.unlit = false;
        mat.base.alpha_mode = cutout_alpha_mode;
        mat.base.opaque_render_method = OpaqueRendererMethod::Forward;
        mat.base.perceptual_roughness = 1.0;
        mat.base.reflectance = 0.02;
    }
    if let Some(mat) = materials.get_mut(&assets.cutout_culled_material) {
        mat.extension.lighting = make_lighting(2.0);
        mat.base.unlit = false;
        mat.base.alpha_mode = cutout_alpha_mode;
        mat.base.opaque_render_method = OpaqueRendererMethod::Forward;
        mat.base.perceptual_roughness = 1.0;
        mat.base.reflectance = 0.02;
    }
    if let Some(mat) = materials.get_mut(&assets.transparent_material) {
        mat.extension.lighting = make_lighting(1.0);
        mat.base.unlit = !uses_shadowed_pbr_path(&settings);
        mat.base.alpha_mode = AlphaMode::Blend;
        mat.base.opaque_render_method = OpaqueRendererMethod::Forward;
        if settings.water_reflections_enabled {
            mat.base.perceptual_roughness = 0.08;
            mat.base.reflectance = 0.9;
        } else {
            mat.base.perceptual_roughness = 1.0;
            mat.base.reflectance = 0.0;
        }
    }

    let shadow_dist_scale = settings.shadow_distance_scale.clamp(0.25, 20.0);
    shadow_map.size = settings.shadow_map_size.clamp(256, 4096) as usize;

    let allow_shadows = settings.shadows_enabled && settings.shadow_cascades > 0;
    let is_fancy = uses_shadowed_pbr_path(&settings);
    let shadow_opacity = settings.shadow_opacity.clamp(0.0, 1.0);
    let sun_color = sun_color_from_warmth(settings.sun_warmth);
    let fill_boost = 1.0 + (1.0 - shadow_opacity) * 1.2;
    let sun_dir = effective_sun_direction(&settings, Some(&world_time));
    let sun_travel_dir = -sun_dir;
    ambient.brightness =
        (settings.ambient_brightness + (1.0 - shadow_opacity) * 0.45).clamp(0.0, 3.0);

    for (mut light, mut light_transform, shadow_light, cascade_cfg) in &mut lights {
        if shadow_light.is_some() {
            light.shadows_enabled = allow_shadows;
            light.illuminance = settings.sun_illuminance;
            light.color = sun_color;
            light.shadow_depth_bias = settings.shadow_depth_bias;
            light.shadow_normal_bias = settings.shadow_normal_bias;
            light_transform.look_to(sun_travel_dir, Vec3::Y);
            if let Some(mut cascade_cfg) = cascade_cfg {
                *cascade_cfg = CascadeShadowConfigBuilder {
                    num_cascades: settings.shadow_cascades.clamp(1, 4) as usize,
                    maximum_distance: settings.shadow_max_distance * shadow_dist_scale,
                    first_cascade_far_bound: settings.shadow_first_cascade_far_bound
                        * shadow_dist_scale,
                    minimum_distance: 0.1,
                    ..default()
                }
                .into();
            }
        } else {
            light.illuminance = if is_fancy {
                settings.fill_illuminance
            } else {
                settings.fill_illuminance * 0.95
            } * fill_boost;
            light.color = sun_color_from_warmth(settings.sun_warmth * 0.35);
            light_transform.look_to(
                Vec3::new(-sun_travel_dir.x, sun_travel_dir.y, -sun_travel_dir.z),
                Vec3::Y,
            );
        }
    }
}

pub fn update_water_animation(
    time: Res<Time>,
    settings: Res<RenderDebugSettings>,
    world_time: Res<WorldTime>,
    assets: Res<ChunkRenderAssets>,
    mut materials: ResMut<Assets<ChunkAtlasMaterial>>,
) {
    let t = time.elapsed_secs_wrapped();
    let fixed_debug = false;
    let reflection_mode = water_reflection_mode(&settings, fixed_debug);
    let plane_y = crate::reflection::DEFAULT_WATER_PLANE_Y;
    let cutout_mode = cutout_alpha_mode(&settings);

    for (handle, pass_mode, force_unlit, alpha_mode) in [
        (
            &assets.opaque_material,
            0.0,
            !uses_shadowed_pbr_path(&settings),
            AlphaMode::Opaque,
        ),
        (&assets.cutout_material, 2.0, false, cutout_mode),
        (&assets.cutout_culled_material, 2.0, false, cutout_mode),
        (
            &assets.transparent_material,
            1.0,
            !uses_shadowed_pbr_path(&settings),
            AlphaMode::Blend,
        ),
    ] {
        if let Some(mat) = materials.get_mut(handle) {
            mat.extension.lighting =
                lighting_uniform_for_mode(&settings, Some(&world_time), pass_mode);
            mat.base.unlit = force_unlit;
            mat.base.alpha_mode = alpha_mode;
            mat.base.opaque_render_method = OpaqueRendererMethod::Forward;
            mat.extension.lighting.water_effects = Vec4::new(
                reflection_mode,
                if fixed_debug {
                    0.0
                } else {
                    settings.water_wave_strength
                },
                if fixed_debug {
                    0.0
                } else {
                    settings.water_wave_speed
                },
                t,
            );
            mat.extension.lighting.water_controls = Vec4::new(
                settings.water_reflection_strength,
                plane_y,
                settings.water_reflection_near_boost,
                if settings.water_reflection_blue_tint {
                    settings.water_reflection_tint_strength
                } else {
                    0.0
                },
            );
            mat.extension.lighting.water_extra = Vec4::new(
                if fixed_debug {
                    0.0
                } else {
                    settings.water_wave_detail_strength
                },
                if fixed_debug {
                    1.0
                } else {
                    settings.water_wave_detail_scale
                },
                if fixed_debug {
                    0.0
                } else {
                    settings.water_wave_detail_speed
                },
                if fixed_debug {
                    0.01
                } else {
                    settings.water_reflection_edge_fade
                },
            );
            mat.extension.lighting.debug_flags = Vec4::new(
                settings.cutout_debug_mode as f32,
                settings.water_reflection_sky_fill,
                settings.shadow_opacity.clamp(0.0, 1.0),
                0.0,
            );
            mat.extension.lighting.grass_overlay_info = assets.grass_overlay_info;
            mat.extension.lighting.reflection_view_proj = Mat4::IDENTITY;
        }
    }
}

fn alpha_mode_code(mode: &AlphaMode) -> u8 {
    match mode {
        AlphaMode::Opaque => 0,
        AlphaMode::Mask(_) => 1,
        AlphaMode::Blend => 2,
        AlphaMode::Premultiplied => 3,
        AlphaMode::Add => 4,
        AlphaMode::Multiply => 5,
        _ => 255,
    }
}

pub fn update_material_debug_stats(
    assets: Res<ChunkRenderAssets>,
    materials: Res<Assets<ChunkAtlasMaterial>>,
    mut perf: ResMut<RenderPerfStats>,
) {
    if let Some(mat) = materials.get(&assets.opaque_material) {
        perf.mat_pass_opaque = mat.extension.lighting.quality_and_water.w;
        perf.mat_alpha_opaque = alpha_mode_code(&mat.base.alpha_mode);
        perf.mat_unlit_opaque = mat.base.unlit;
    }
    if let Some(mat) = materials.get(&assets.cutout_material) {
        perf.mat_pass_cutout = mat.extension.lighting.quality_and_water.w;
        perf.mat_alpha_cutout = alpha_mode_code(&mat.base.alpha_mode);
        perf.mat_unlit_cutout = mat.base.unlit;
    }
    if let Some(mat) = materials.get(&assets.cutout_culled_material) {
        perf.mat_pass_cutout_culled = mat.extension.lighting.quality_and_water.w;
        perf.mat_alpha_cutout_culled = alpha_mode_code(&mat.base.alpha_mode);
        perf.mat_unlit_cutout_culled = mat.base.unlit;
    }
    if let Some(mat) = materials.get(&assets.transparent_material) {
        perf.mat_pass_transparent = mat.extension.lighting.quality_and_water.w;
        perf.mat_alpha_transparent = alpha_mode_code(&mat.base.alpha_mode);
        perf.mat_unlit_transparent = mat.base.unlit;
    }
}
