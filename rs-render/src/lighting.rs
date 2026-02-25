use bevy::core_pipeline::{
    fxaa::{Fxaa, Sensitivity},
    prepass::DepthPrepass,
    smaa::{Smaa, SmaaPreset},
};
use bevy::pbr::{
    CascadeShadowConfig, CascadeShadowConfigBuilder, DirectionalLightShadowMap,
    OpaqueRendererMethod,
    ScreenSpaceAmbientOcclusion,
};
use bevy::prelude::*;
use bevy::render::view::Msaa;

use crate::chunk::{AtlasLightingUniform, ChunkAtlasMaterial, ChunkRenderAssets};
use crate::components::ShadowCasterLight;
use crate::debug::{AntiAliasingMode, RenderDebugSettings, RenderPerfStats};
use crate::reflection::DEFAULT_WATER_PLANE_Y;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum LightingQualityPreset {
    Fast,
    Standard,
    FancyLow,
    FancyHigh,
}

impl Default for LightingQualityPreset {
    fn default() -> Self {
        Self::Standard
    }
}

impl LightingQualityPreset {
    pub const ALL: [Self; 4] = [Self::Fast, Self::Standard, Self::FancyLow, Self::FancyHigh];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Fast => "Fast",
            Self::Standard => "Standard",
            Self::FancyLow => "Fancy Low",
            Self::FancyHigh => "Fancy High",
        }
    }

    pub const fn as_options_value(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Standard => "standard",
            Self::FancyLow => "fancy_low",
            Self::FancyHigh => "fancy_high",
        }
    }

    pub fn from_options_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "fast" => Some(Self::Fast),
            "standard" => Some(Self::Standard),
            "fancy_low" | "fancylow" => Some(Self::FancyLow),
            "fancy_high" | "fancyhigh" => Some(Self::FancyHigh),
            _ => None,
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum ShadowQualityPreset {
    Off,
    Low,
    Medium,
    High,
}

impl Default for ShadowQualityPreset {
    fn default() -> Self {
        Self::Medium
    }
}

impl ShadowQualityPreset {
    pub const ALL: [Self; 4] = [Self::Off, Self::Low, Self::Medium, Self::High];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }

    pub const fn as_options_value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn from_options_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "off" => Some(Self::Off),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }
}

pub const fn uses_shadowed_pbr_path(settings: &RenderDebugSettings) -> bool {
    settings.enable_pbr_terrain_lighting
}

fn cutout_alpha_mode(settings: &RenderDebugSettings) -> AlphaMode {
    let _ = settings;
    // Use explicit alpha-mask for cutout vegetation:
    // keeps deterministic depth ordering while clipping transparent texels in
    // both forward and prepass paths.
    AlphaMode::Mask(0.5)
}

fn water_reflection_mode(settings: &RenderDebugSettings, fixed_debug: bool) -> f32 {
    if fixed_debug {
        0.0
    } else if settings.water_reflections_enabled && settings.water_reflection_screen_space {
        4.0
    } else {
        0.0
    }
}

pub fn lighting_uniform_for_mode(
    settings: &RenderDebugSettings,
    pass_mode: f32, // 0 opaque, 1 transparent(water), 2 cutout
) -> AtlasLightingUniform {
    let fixed_debug = false;
    let az = settings.sun_azimuth_deg.to_radians();
    let el = settings.sun_elevation_deg.to_radians();
    let sun_dir = Vec3::new(el.cos() * az.cos(), el.sin(), el.cos() * az.sin())
        .normalize_or_zero();
    let quality_mode = if fixed_debug {
        0.0
    } else {
        settings.shader_quality_mode.clamp(0, 3) as f32
    };
    AtlasLightingUniform {
        sun_dir_and_strength: Vec4::new(
            sun_dir.x,
            sun_dir.y,
            sun_dir.z,
            settings.sun_strength,
        ),
        ambient_and_fog: Vec4::new(
            settings.ambient_strength,
            if fixed_debug { 0.0 } else { settings.fog_density },
            if fixed_debug { 10_000.0 } else { settings.fog_start },
            if fixed_debug { 10_001.0 } else { settings.fog_end },
        ),
        quality_and_water: Vec4::new(
            quality_mode,
            settings.water_absorption,
            settings.water_fresnel,
            pass_mode,
        ),
        color_grading: Vec4::new(
            if fixed_debug {
                1.0
            } else {
                settings.color_saturation
            },
            if fixed_debug {
                1.0
            } else {
                settings.color_contrast
            },
            if fixed_debug {
                0.0
            } else {
                settings.color_brightness
            },
            if fixed_debug { 1.0 } else { settings.color_gamma },
        ),
        water_effects: Vec4::new(
            water_reflection_mode(settings, fixed_debug),
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
            0.0,
        ),
        water_controls: Vec4::new(
            settings.water_reflection_strength,
            DEFAULT_WATER_PLANE_Y,
            settings.water_reflection_near_boost,
            if settings.water_reflection_blue_tint {
                settings.water_reflection_tint_strength
            } else {
                0.0
            },
        ),
        water_extra: Vec4::new(
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
        ),
        ssr_params: Vec4::new(
            if fixed_debug {
                0.0
            } else {
                settings.water_ssr_steps.clamp(4, 64) as f32
            },
            if fixed_debug {
                0.2
            } else {
                settings.water_ssr_thickness.clamp(0.02, 2.0)
            },
            if fixed_debug {
                40.0
            } else {
                settings.water_ssr_max_distance.clamp(4.0, 400.0)
            },
            if fixed_debug {
                1.0
            } else {
                settings.water_ssr_stride.clamp(0.2, 8.0)
            },
        ),
        debug_flags: Vec4::new(
            settings.cutout_debug_mode as f32,
            settings.water_reflection_sky_fill,
            0.0,
            0.0,
        ),
        grass_overlay_info: Vec4::new(f32::NAN, f32::NAN, f32::NAN, f32::NAN),
        reflection_view_proj: Mat4::IDENTITY,
    }
}

pub fn apply_lighting_quality(
    settings: Res<RenderDebugSettings>,
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
        let mut u = lighting_uniform_for_mode(&settings, pass_mode);
        u.grass_overlay_info = grass_overlay_info;
        u
    };
    let material_key = (
        settings.shader_quality_mode,
        settings.enable_pbr_terrain_lighting,
        settings.material_rebuild_nonce,
    );
    let recreate_materials = last_material_key
        .map(|k| k != material_key)
        .unwrap_or(true);
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
    }
    if let Some(mat) = materials.get_mut(&assets.cutout_material) {
        mat.extension.lighting = make_lighting(2.0);
        mat.base.unlit = false;
        mat.base.alpha_mode = cutout_alpha_mode;
        mat.base.opaque_render_method = OpaqueRendererMethod::Forward;
    }
    if let Some(mat) = materials.get_mut(&assets.cutout_culled_material) {
        mat.extension.lighting = make_lighting(2.0);
        mat.base.unlit = false;
        mat.base.alpha_mode = cutout_alpha_mode;
        mat.base.opaque_render_method = OpaqueRendererMethod::Forward;
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
    let az = settings.sun_azimuth_deg.to_radians();
    let el = settings.sun_elevation_deg.to_radians();
    let sun_dir = Vec3::new(el.cos() * az.cos(), el.sin(), el.cos() * az.sin())
        .normalize_or_zero();
    let sun_travel_dir = -sun_dir;
    ambient.brightness = settings.ambient_brightness;

    for (mut light, mut light_transform, shadow_light, cascade_cfg) in &mut lights {
        if shadow_light.is_some() {
            light.shadows_enabled = allow_shadows;
            light.illuminance = settings.sun_illuminance;
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
            };
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
    assets: Res<ChunkRenderAssets>,
    mut materials: ResMut<Assets<ChunkAtlasMaterial>>,
) {
    let t = time.elapsed_secs_wrapped();
    let fixed_debug = false;
    let reflection_mode = water_reflection_mode(&settings, fixed_debug);
    let plane_y = DEFAULT_WATER_PLANE_Y;
    let cutout_alpha_mode = cutout_alpha_mode(&settings);

    for (handle, pass_mode, force_unlit, alpha_mode) in [
        (&assets.opaque_material, 0.0, !uses_shadowed_pbr_path(&settings), AlphaMode::Opaque),
        (
            &assets.cutout_material,
            2.0,
            false,
            cutout_alpha_mode,
        ),
        (
            &assets.cutout_culled_material,
            2.0,
            false,
            cutout_alpha_mode,
        ),
        (&assets.transparent_material, 1.0, !uses_shadowed_pbr_path(&settings), AlphaMode::Blend),
    ] {
        if let Some(mat) = materials.get_mut(handle) {
            // Rebuild the full uniform every frame to prevent stale pass specialization
            // when quality presets are switched at runtime.
            mat.extension.lighting = lighting_uniform_for_mode(&settings, pass_mode);
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
            mat.extension.lighting.water_controls =
                Vec4::new(
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
            mat.extension.lighting.debug_flags =
                Vec4::new(
                    settings.cutout_debug_mode as f32,
                    settings.water_reflection_sky_fill,
                    0.0,
                    0.0,
                );
            mat.extension.lighting.grass_overlay_info = assets.grass_overlay_info;
            mat.extension.lighting.reflection_view_proj = Mat4::IDENTITY;
        }
    }
}

pub fn apply_antialiasing(
    settings: Res<RenderDebugSettings>,
    mut camera_query: Query<
        (Entity, Option<&mut Fxaa>, Option<&mut Smaa>, &mut Msaa),
        With<crate::components::PlayerCamera>,
    >,
    mut commands: Commands,
) {
    if !settings.is_changed() {
        return;
    }
    let Ok((camera_entity, fxaa_opt, smaa_opt, mut msaa)) = camera_query.single_mut() else {
        return;
    };

    match settings.aa_mode {
        AntiAliasingMode::Off => {
            *msaa = Msaa::Off;
            if let Some(mut fxaa) = fxaa_opt {
                fxaa.enabled = false;
            }
            if smaa_opt.is_some() {
                commands.entity(camera_entity).remove::<Smaa>();
            }
        }
        AntiAliasingMode::Fxaa => {
            *msaa = Msaa::Off;
            if let Some(mut fxaa) = fxaa_opt {
                fxaa.enabled = true;
                fxaa.edge_threshold = Sensitivity::Ultra;
                fxaa.edge_threshold_min = Sensitivity::High;
            } else {
                commands.entity(camera_entity).insert(Fxaa {
                    enabled: true,
                    edge_threshold: Sensitivity::Ultra,
                    edge_threshold_min: Sensitivity::High,
                });
            }
            if smaa_opt.is_some() {
                commands.entity(camera_entity).remove::<Smaa>();
            }
        }
        AntiAliasingMode::SmaaHigh => {
            *msaa = Msaa::Off;
            if let Some(mut fxaa) = fxaa_opt {
                fxaa.enabled = false;
            }
            if let Some(mut smaa) = smaa_opt {
                smaa.preset = SmaaPreset::High;
            } else {
                commands.entity(camera_entity).insert(Smaa {
                    preset: SmaaPreset::High,
                });
            }
        }
        AntiAliasingMode::SmaaUltra => {
            *msaa = Msaa::Off;
            if let Some(mut fxaa) = fxaa_opt {
                fxaa.enabled = false;
            }
            if let Some(mut smaa) = smaa_opt {
                smaa.preset = SmaaPreset::Ultra;
            } else {
                commands.entity(camera_entity).insert(Smaa {
                    preset: SmaaPreset::Ultra,
                });
            }
        }
        AntiAliasingMode::Msaa4 => {
            *msaa = Msaa::Sample4;
            if let Some(mut fxaa) = fxaa_opt {
                fxaa.enabled = true;
                fxaa.edge_threshold = Sensitivity::High;
                fxaa.edge_threshold_min = Sensitivity::Medium;
            } else {
                commands.entity(camera_entity).insert(Fxaa {
                    enabled: true,
                    edge_threshold: Sensitivity::High,
                    edge_threshold_min: Sensitivity::Medium,
                });
            }
            if smaa_opt.is_some() {
                commands.entity(camera_entity).remove::<Smaa>();
            }
        }
        AntiAliasingMode::Msaa8 => {
            *msaa = Msaa::Sample8;
            if let Some(mut fxaa) = fxaa_opt {
                fxaa.enabled = true;
                fxaa.edge_threshold = Sensitivity::High;
                fxaa.edge_threshold_min = Sensitivity::Medium;
            } else {
                commands.entity(camera_entity).insert(Fxaa {
                    enabled: true,
                    edge_threshold: Sensitivity::High,
                    edge_threshold_min: Sensitivity::Medium,
                });
            }
            if smaa_opt.is_some() {
                commands.entity(camera_entity).remove::<Smaa>();
            }
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

pub fn apply_ssao_quality(
    settings: Res<RenderDebugSettings>,
    camera_query: Query<Entity, With<crate::components::PlayerCamera>>,
    mut commands: Commands,
) {
    if !settings.is_changed() {
        return;
    }
    let Ok(camera_entity) = camera_query.single() else {
        return;
    };
    let _ = settings;
    // IMPORTANT:
    // SSAO forces Bevy's prepass path. Our atlas cutout is implemented in the
    // custom material shader, but the prepass doesn't apply the same atlas alpha
    // clip, which writes full-quad depth for cutout meshes (leaves/tallgrass/etc.).
    // Result: hole pixels show clear gray instead of geometry behind.
    //
    // Until we add a dedicated cutout-aware prepass shader path, keep SSAO disabled.
    commands
        .entity(camera_entity)
        .remove::<ScreenSpaceAmbientOcclusion>();
}

pub fn apply_depth_prepass_for_ssr(
    settings: Res<RenderDebugSettings>,
    camera_query: Query<(Entity, Option<&DepthPrepass>), With<crate::components::PlayerCamera>>,
    mut commands: Commands,
) {
    let Ok((camera_entity, has_depth_prepass)) = camera_query.single() else {
        return;
    };
    let want_depth_prepass = settings.water_reflections_enabled && settings.water_reflection_screen_space;
    match (want_depth_prepass, has_depth_prepass.is_some()) {
        (true, false) => {
            commands.entity(camera_entity).insert(DepthPrepass);
        }
        (false, true) => {
            commands.entity(camera_entity).remove::<DepthPrepass>();
        }
        _ => {}
    }
}
